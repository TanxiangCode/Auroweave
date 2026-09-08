/// Auroweave — Rust 库入口
/// 作者: TanXiang
pub mod error;
pub mod commands;
pub mod core;
pub mod fs_utils;
pub mod speedtest;
pub mod system;

use core::sidecar::SidecarManager;
use speedtest::scheduler::SpeedTestScheduler;
use commands::unlock_check::UnlockCheckScheduler;
use std::sync::Arc;
use tauri::Manager;

/// 获取统一数据根目录：Windows 为 C:\ProgramData\Auroweave，macOS 为 ~/Library/Application Support/Auroweave
/// 服务、GUI 主程序共用此目录，SYSTEM 用户和普通用户均可访问。
pub fn get_data_root() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        std::path::PathBuf::from(program_data).join("Auroweave")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        std::path::PathBuf::from(home).join("Library").join("Application Support").join("Auroweave")
    }
    #[cfg(target_os = "linux")]
    {
        std::path::PathBuf::from("/var/lib/Auroweave")
    }
}

/// 获取配置子目录：%ProgramData%\Auroweave\config\
pub fn get_config_dir() -> std::path::PathBuf {
    get_data_root().join("config")
}

/// 获取日志子目录：%ProgramData%\Auroweave\logs\
pub fn get_log_dir() -> std::path::PathBuf {
    get_data_root().join("logs")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 确保日志目录存在
    let log_dir = get_log_dir();
    let _ = std::fs::create_dir_all(&log_dir);

    let sidecar_manager = Arc::new(SidecarManager::new());
    let speedtest_scheduler = Arc::new(SpeedTestScheduler::new());
    let unlock_scheduler = Arc::new(UnlockCheckScheduler::new());
    // test-core 管理器：Exit 钩子持有（退出清理短命测试内核）；
    // setup 与 run 两个 move 闭包各自捕获一个克隆
    let test_core_manager = Arc::new(core::test_core::TestCoreManager::new());
    let test_core_manager_for_exit = test_core_manager.clone();

    tauri::Builder::default()
        .manage(sidecar_manager.clone())
        .manage(speedtest_scheduler.clone())
        .manage(unlock_scheduler.clone())
        // 注册单例插件，确保只运行一个实例
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // 如果尝试启动新实例，将其聚焦（可以触发某些事件）
            let _ = app.get_webview_window("main").map(|w| {
                let _ = w.set_focus();
            });
        }))
        // tauri-plugin-log：前端+后端统一写入同一日志文件
        .plugin(
            tauri_plugin_log::Builder::new()
                // 输出到文件（追加模式）
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: log_dir.clone(),
                        file_name: Some("auroweave".to_string()),
                    }
                ))
                // 同时输出到 Webview DevTools 控制台
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Webview
                ))
                // 日志级别：默认 Trace
                .level(log::LevelFilter::Trace)
                // Tauri 内部框架仅记录 Warn 以上，减少噪音
                .level_for("tauri", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .build()
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // 开机自启：macOS 走 LaunchAgent（login item），初始状态由 setup 阶段按设置同步
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::proxy::proxy_get_groups,
            commands::proxy::proxy_get_group_nodes,
            commands::proxy::proxy_select_node,
            commands::proxy::proxy_get_mode,
            commands::proxy::proxy_set_mode,
            commands::proxy::proxy_get_singbox_version,
            commands::proxy::proxy_close_connection,
            commands::proxy::proxy_close_all_connections,
            commands::proxy::proxy_connectivity_check,
            commands::proxy::sysproxy_set,
            commands::proxy::app_restart_as_admin,
            commands::subscription::subscription_import,
            commands::subscription::subscription_import_content,
            commands::subscription::subscription_update_meta,
            commands::subscription::subscription_get_all,

            commands::subscription::subscription_delete,
            commands::subscription::subscription_delete_all,
            commands::subscription::subscription_refresh,
            commands::subscription::subscription_activate,
            commands::subscription::subscription_inspect,
            commands::subscription::ruleset_force_update,
            commands::subscription::ruleset_get_status,
            commands::settings::settings_get_all,
            commands::settings::settings_save,
            commands::settings::settings_inject_terminal_proxy,
            commands::settings::settings_export_diagnostic_log,
            commands::settings::tun_set_enabled,
            commands::settings::service_query_status,
            commands::settings::service_install,
            commands::settings::service_uninstall,
            commands::settings::service_start,
            commands::settings::service_stop,
            commands::settings::service_read_log,
            commands::speedtest::speedtest_run_latency,
            commands::speedtest::speedtest_run_single,
            commands::speedtest::speedtest_run_batch,
            commands::speedtest::speedtest_cancel_batch,
            commands::speedtest::speedtest_get_results,
            commands::speedtest::speedtest_get_history,
            // AI 服务解锁检测
            commands::unlock_check::unlock_check_single,
            commands::unlock_check::unlock_check_batch,
            commands::unlock_check::unlock_check_cancel,
            commands::unlock_check::unlock_check_get_latest,
            commands::routing::routing_get_processes,
            commands::routing::routing_get_app_rules,
            commands::routing::routing_save_app_rule,
            commands::routing::routing_get_custom_rules,
            commands::routing::routing_save_custom_rules,
            commands::routing::routing_add_custom_rule,
            commands::routing::routing_delete_custom_rule,
            commands::routing::routing_export_rules,
            commands::routing::routing_import_rules,
            // 日志管理命令

            commands::logging::log_read_app,
            commands::logging::log_read_service,
            commands::logging::log_read_kernel,
            commands::logging::log_clear_all,
            commands::settings::core_query_running,
            // 流量统计命令
            commands::stats::get_traffic_history,
            commands::stats::get_app_traffic_stats,
            commands::stats::stats_clear_all,
            commands::settings::settings_restore_backup,
            // 分组测速配置更新（GroupEditModal 保存）
            commands::settings::group_update_config,
            // sing-box 内核版本检测与在线更新
            commands::singbox_update::core_check_singbox_update,
            commands::singbox_update::core_upgrade_singbox,
            // 配置编辑器（plan-Q）：schema 导出 + 安全编辑
            commands::config_editor::config_export_schema,
            commands::config_editor::config_editor_load,
            commands::config_editor::config_editor_save,
            commands::config_editor::config_editor_restart_core,
            // ClashAPI 访问令牌（供前端 WebSocket 鉴权）
            core::clash_api::core_get_clash_secret,
            // 托盘实时网速同步
            system::tray::tray_update_traffic,
        ])


        .setup(move |app| {
            let main_window = app.get_webview_window("main")
                .expect("找不到主窗口，请检查 tauri.conf.json 中的窗口配置");

            // 初始化系统托盘与右键快捷菜单
            if let Err(e) = system::tray::setup_tray(app.handle()) {
                log::error!("[tray] 初始化托盘失败: {}", e);
            }

            // 开机自启状态校准：settings.auto_start 与系统注册（LaunchAgent）对齐
            let settings_for_autostart = commands::settings::settings_get_internal(app.handle());
            if let Err(e) = system::autostart::sync_autostart(app.handle(), settings_for_autostart.auto_start) {
                log::error!("[autostart] 启动时同步自启状态失败: {}", e);
            }

            // 监听窗口关闭事件：拦截右上角 X，改为最小化至托盘
            let handle_for_window = app.handle().clone();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let settings = commands::settings::settings_get_internal(&handle_for_window);
                    if settings.minimize_to_tray {
                        api.prevent_close();
                        system::tray::hide_main_window(&handle_for_window);
                    }
                }
            });

            // 执行旧数据迁移（首次启动时将 %APPDATA% 数据迁移到 %ProgramData%）
            system::startup::migrate_legacy_data(app.handle());

            // 初始化全局 ClashAPI 端口
            let (_, clash_port) = speedtest::get_configured_ports(app.handle());
            core::clash_api::set_clash_api_port(clash_port);

            log::info!("[app] Auroweave 启动 | 数据根目录: {:?}", get_data_root());
            log::info!("[app] Tauri 主窗口已创建");

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = system::startup::apply_core_mode_with_fallback(&app_handle).await {
                    log::error!("[app] 核心自愈与拉起发生错误: {}", e);
                }

                // 若开启了开机自启静默启动，隐藏窗口
                let settings = commands::settings::settings_get_internal(&app_handle);
                if settings.start_minimized {
                    system::tray::hide_main_window(&app_handle);
                }
            });

            // 启动后台流量监控
            core::traffic_monitor::start_monitor(app.handle().clone());

            // 启动后台订阅自动静默更新调度器
            commands::subscription::start_auto_update_scheduler(app.handle().clone());

            // 启动系统代理守护（30s 一拍，外部应用篡改代理设置时自动恢复；
            // 冲突仲裁由期望状态标记承担，见 system::proxy_guard 模块注释）
            system::proxy_guard::start_guard(app.handle().clone());

            Ok(())

        })

        .build(tauri::generate_context!())
        .expect("Tauri 构建失败")
        .run(move |app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                log::info!("[app] 程序正在退出，清理网络代理...");
                // 使用静默模式清理代理，避免退出时弹出 macOS 密码框阻塞退出流程
                let _ = system::sysproxy::set_system_proxy_silent(false, 0);
                // test-core 短命测试内核：退出时必须清理（与运行模式无关，
                // 服务模式下它同样可能因批量检测而存活）
                {
                    let core = test_core_manager_for_exit.clone();
                    tauri::async_runtime::block_on(async move {
                        core.stop().await;
                    });
                }
                let settings = commands::settings::settings_get_internal(app_handle);
                if settings.core.run_mode == "service" {
                    log::info!("[app] 服务模式退出：停止系统服务");
                    let _ = system::service_control::stop_service();
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                } else {
                    log::info!("[app] 直接运行模式退出：停止 sing-box");
                    // Windows: 停止计划任务 TUN；macOS/Linux: 仅停止 sidecar
                    #[cfg(target_os = "windows")]
                    let _ = system::service_control::stop_direct_tun_task();
                    let sidecar_manager = app_handle.state::<std::sync::Arc<SidecarManager>>().inner().clone();
                    // 使用静默停止：macOS TUN 模式下不弹出密码框，
                    // root 进程可能继续运行但会在下次启动时自动清理
                    tauri::async_runtime::block_on(async move {
                        let _ = sidecar_manager.stop_silent().await;
                    });
                }
            }
        });
}
