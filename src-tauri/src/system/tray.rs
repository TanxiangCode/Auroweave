use std::sync::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tracing::info;
use crate::commands::settings::settings_get_internal;

pub const TRAY_ID: &str = "auroweave-tray";

/// 托盘设置 TTL 缓存：避免每次流量刷新（最高频 2s/次）都重读 settings.json
///
/// 结构: (show_tray_speed 是否显示, clash_api_port 端口, 缓存写入时刻)
static TRAY_SETTINGS_CACHE: Mutex<Option<(bool, u16, std::time::Instant)>> = Mutex::new(None);

/// TTL 缓存有效期：30 秒（设置变更最多延迟 30 秒反映到托盘，换来高频路径免读盘）
const TRAY_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

/// 读取托盘所需设置（show_tray_speed + clash_api_port），带 30s TTL 缓存
fn tray_settings_cached(app_handle: &AppHandle) -> (bool, u16) {
    let mut cache = TRAY_SETTINGS_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((show, port, at)) = cache.as_ref() {
        if at.elapsed() < TRAY_CACHE_TTL {
            return (*show, *port);
        }
    }
    let settings = settings_get_internal(app_handle);
    let fresh = (settings.show_tray_speed, settings.clash_api_port);
    *cache = Some((fresh.0, fresh.1, std::time::Instant::now()));
    fresh
}

/// 初始化系统托盘与右键菜单
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItemBuilder::with_id("show_window", "显示主界面").build(app)?;
    let settings_item = MenuItemBuilder::with_id("open_settings", "偏好设置...").build(app)?;

    // 代理模式切换子菜单
    let mode_rule = MenuItemBuilder::with_id("mode_rule", "规则分流 (Rule)").build(app)?;
    let mode_global = MenuItemBuilder::with_id("mode_global", "全局代理 (Global)").build(app)?;
    let mode_direct = MenuItemBuilder::with_id("mode_direct", "大陆直连 (Direct)").build(app)?;

    let mode_submenu = SubmenuBuilder::new(app, "代理分流模式")
        .item(&mode_rule)
        .item(&mode_global)
        .item(&mode_direct)
        .build()?;

    // 系统代理与 TUN 切换
    let sysproxy_item = MenuItemBuilder::with_id("toggle_sysproxy", "开启系统代理 (System Proxy)").build(app)?;
    let restart_item = MenuItemBuilder::with_id("restart_kernel", "重启内核服务 (Restart Core)").build(app)?;

    let quit_item = MenuItemBuilder::with_id("quit_app", "退出 Auroweave (Quit)").build(app)?;


    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let separator3 = PredefinedMenuItem::separator(app)?;

    let menu = MenuBuilder::new(app)
        .item(&show_item)
        .item(&settings_item)
        .item(&separator1)
        .item(&mode_submenu)
        .item(&sysproxy_item)
        .item(&separator2)
        .item(&restart_item)
        .item(&separator3)
        .item(&quit_item)
        .build()?;


    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("Auroweave - 下一代极简跨平台代理客户端")
        .show_menu_on_left_click(false);

    // 加载专属托盘图标 (macOS 模板图标自适应明暗菜单栏)
    let icon_bytes = include_bytes!("../../icons/tray-icon.png");
    if let Ok(img) = tauri::image::Image::from_bytes(icon_bytes) {
        builder = builder.icon(img).icon_as_template(true);
    } else if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _tray = builder
        .on_menu_event(|app_handle, event| {
            let id = event.id.as_ref();
            match id {
                "show_window" => {
                    show_main_window(app_handle);
                }
                "open_settings" => {
                    show_main_window(app_handle);
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.eval("window.location.hash = '#/settings'");
                    }
                }
                "mode_rule" => {
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::proxy::proxy_set_mode(handle, "rule".to_string()).await;
                    });
                }
                "mode_global" => {
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::proxy::proxy_set_mode(handle, "global".to_string()).await;
                    });
                }
                "mode_direct" => {
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::commands::proxy::proxy_set_mode(handle, "direct".to_string()).await;
                    });
                }
                "toggle_sysproxy" => {
                    // 菜单切换语义：读取当前系统代理状态后翻转（开→关→开），
                    // 而非旧实现的"点击永远是开启"
                    let currently_enabled = crate::system::sysproxy::get_system_proxy_status();
                    let target_enabled = !currently_enabled;
                    if target_enabled {
                        let (port, _) = crate::speedtest::get_configured_ports(app_handle);
                        match crate::system::sysproxy::set_system_proxy(true, port) {
                            Ok(_) => info!("[tray] 系统代理已开启: port={}", port),
                            Err(e) => info!("[tray] 开启系统代理失败: {}", e),
                        }
                    } else {
                        match crate::system::sysproxy::set_system_proxy(false, 0) {
                            Ok(_) => info!("[tray] 系统代理已关闭"),
                            Err(e) => info!("[tray] 关闭系统代理失败: {}", e),
                        }
                    }
                }
                "restart_kernel" => {
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::system::startup::apply_core_mode_with_fallback(&handle).await;
                        info!("[tray] 内核已手动重启");
                    });
                }
                "quit_app" => {
                    info!("[tray] 用户点击退出应用");
                    app_handle.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    info!("[tray] 系统托盘初始化完成");
    start_tray_traffic_ticker(app.clone());

    Ok(())
}

/// 显示并聚焦主窗口
pub fn show_main_window(app_handle: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        // 恢复 macOS 程序坞图标可见
        let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Regular);
    }

    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 隐藏主窗口（并根据设置决定是否隐藏 macOS 程序坞图标）
pub fn hide_main_window(app_handle: &AppHandle) {
    let settings = settings_get_internal(app_handle);

    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.hide();
    }

    #[cfg(target_os = "macos")]
    {
        if settings.hide_dock_on_close {
            // 隐藏程序坞图标，转为纯托盘后台常驻模式
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" {
    fn macos_set_tray_attributed_title(text: *const std::os::raw::c_char);
}

/// 刷新托盘网速显示核心逻辑
pub fn update_tray_speed_display(app_handle: &AppHandle, up_bps: u64, down_bps: u64, is_active: bool) {
    // 使用 30s TTL 缓存读取（show_tray_speed, 端口），避免 1~2s 高频读盘
    let (show_tray_speed, _) = tray_settings_cached(app_handle);
    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        if show_tray_speed && is_active {
            #[cfg(target_os = "macos")]
            {
                // 双排显示：第一行上行速度，第二行下行速度，右侧速度文本右对齐 (7.3pt 等宽数字)
                let title_text = format_dual_line_speed(up_bps, down_bps);
                let _ = tray.set_title(Some(&title_text));
                let c_text = std::ffi::CString::new(title_text).unwrap_or_default();
                unsafe {
                    macos_set_tray_attributed_title(c_text.as_ptr());
                }
            }

            let up_compact = format_speed_compact(up_bps);
            let down_compact = format_speed_compact(down_bps);
            let tooltip_text = format!("Auroweave\n↑ {}\n↓ {}", up_compact, down_compact);
            let _ = tray.set_tooltip(Some(tooltip_text));
        } else {
            #[cfg(target_os = "macos")]
            {
                let _ = tray.set_title(None::<&str>);
                let c_empty = std::ffi::CString::new("").unwrap_or_default();
                unsafe {
                    macos_set_tray_attributed_title(c_empty.as_ptr());
                }
            }
            let _ = tray.set_tooltip(Some("Auroweave - 下一代极简跨平台代理客户端"));
        }
    }
}


/// 格式化双排网速（左侧箭头，右侧速度数值和单位右对齐）
fn format_dual_line_speed(up_bps: u64, down_bps: u64) -> String {
    let up_raw = format_speed_compact(up_bps);
    let down_raw = format_speed_compact(down_bps);

    // 对齐右侧字符串宽度
    let max_len = up_raw.chars().count().max(down_raw.chars().count());
    let up_aligned = format!("{:>width$}", up_raw, width = max_len);
    let down_aligned = format!("{:>width$}", down_raw, width = max_len);

    format!("↑ {}\n↓ {}", up_aligned, down_aligned)
}

/// 紧凑网速格式化（带统一单位与数字排版）
fn format_speed_compact(bytes_per_sec: u64) -> String {
    if bytes_per_sec < 1024 {
        format!("{} B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        let kb = bytes_per_sec as f64 / 1024.0;
        if kb < 100.0 {
            format!("{:.1} KB/s", kb)
        } else {
            format!("{:.0} KB/s", kb)
        }
    } else if bytes_per_sec < 1024 * 1024 * 1024 {
        let mb = bytes_per_sec as f64 / (1024.0 * 1024.0);
        if mb < 100.0 {
            format!("{:.1} MB/s", mb)
        } else {
            format!("{:.0} MB/s", mb)
        }
    } else {
        let gb = bytes_per_sec as f64 / (1024.0 * 1024.0 * 1024.0);
        format!("{:.2} GB/s", gb)
    }
}

/// 前端 WebSocket 流量直通更新 IPC 命令
#[tauri::command]
pub fn tray_update_traffic(app_handle: AppHandle, up: u64, down: u64, active: Option<bool>) {
    let is_active = active.unwrap_or(true);
    update_tray_speed_display(&app_handle, up, down, is_active);
}

/// 实时网速后台长连接轮询器（独立于前端视窗的双重保障）
fn start_tray_traffic_ticker(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600))
            .build()
            .unwrap_or_default();

        loop {
            // 端口读取走 TTL 缓存，避免长连接重试循环反复读盘
            let (_, port) = tray_settings_cached(&app_handle);
            let url = format!("http://127.0.0.1:{}/traffic", port);

            // 连接长连接 stream
            if let Ok(mut resp) = client.get(&url).send().await {
                let mut buffer = String::new();
                // 节流：流每秒推一拍，NSStatusItem 每秒 set_title 有主线程成本
                // （前端 IPC 路为 3s 节流；此处对齐 3s，双路节奏一致且降 2/3 更新量）
                let mut last_update = std::time::Instant::now() - std::time::Duration::from_secs(10);
                while let Ok(Some(chunk)) = resp.chunk().await {
                    if let Ok(text) = std::str::from_utf8(&chunk) {
                        buffer.push_str(text);
                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim().to_string();
                            buffer = buffer[pos + 1..].to_string();

                            if !line.is_empty() {
                                #[derive(serde::Deserialize)]
                                struct TrafficItem {
                                    up: u64,
                                    down: u64,
                                }
                                if let Ok(traffic) = serde_json::from_str::<TrafficItem>(&line) {
                                    if last_update.elapsed() >= std::time::Duration::from_secs(3) {
                                        last_update = std::time::Instant::now();
                                        update_tray_speed_display(&app_handle, traffic.up, traffic.down, true);
                                    }
                                }
                            }
                        }
                    }
                }
            }


            // 若连接断开，等待 2 秒后重试
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
}


