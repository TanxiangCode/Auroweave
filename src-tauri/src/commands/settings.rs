/// IPC 命令 — 应用设置
/// 作者: TanXiang
use crate::error::ApiResponse;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoreServiceSettings {
    #[serde(rename = "installedVersion")]
    pub installed_version: Option<String>,
    #[serde(rename = "lastKnownStatus")]
    pub last_known_status: String,
    #[serde(rename = "lastFallbackReason")]
    pub last_fallback_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoreSettings {
    #[serde(rename = "runMode")]
    pub run_mode: String,
    pub service: CoreServiceSettings,
}

fn default_core_settings() -> CoreSettings {
    CoreSettings {
        run_mode: "local".to_string(),
        service: CoreServiceSettings {
            installed_version: None,
            last_known_status: "not_installed".to_string(),
            last_fallback_reason: None,
        },
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub theme: String,
    pub language: String,
    pub proxy_mode: String,
    pub auto_start: bool,
    pub tun_enabled: bool,
    /// TUN 虚拟网卡名称，显示 in Windows 网络适配器列表中，默认 Auroweave
    pub tun_interface_name: String,
    pub topology_enabled: bool,
    pub performance_mode: bool,
    pub command_palette_hotkey: String,
    pub speed_test_urls: Vec<String>,
    pub auto_group_on_import: bool,

    // 网络代理端口与测速超时可配置项
    pub mixed_port: u16,
    pub clash_api_port: u16,
    pub speed_test_url: String,
    pub speed_test_timeout_secs: u64,
    pub connection_timeout_secs: u64,
    pub enable_app_traffic_tracking: bool,

    #[serde(default = "default_core_settings")]
    pub core: CoreSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "zh-CN".to_string(),
            proxy_mode: "rule".to_string(),
            auto_start: false,
            tun_enabled: false,
            tun_interface_name: "Auroweave".to_string(),
            topology_enabled: false,
            performance_mode: false,
            command_palette_hotkey: "CommandOrControl+Space".to_string(),
            speed_test_urls: vec![
                "https://speed.cloudflare.com/__down?bytes=25000000".to_string(),
                "https://fast.com".to_string(),
            ],
            auto_group_on_import: true,

            // 默认端口与超时设定
            mixed_port: 8890,
            clash_api_port: 9090,
            speed_test_url: "https://speed.cloudflare.com/__down?bytes=25000000".to_string(),
            speed_test_timeout_secs: 5,
            connection_timeout_secs: 15,
            enable_app_traffic_tracking: true,
            core: default_core_settings(),
        }
    }
}

fn get_settings_path() -> PathBuf {
    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);
    config_dir.join("settings.json")
}

/// 内部加载设置函数 (供各 Rust 模块使用)
pub fn settings_get_internal(_app_handle: &tauri::AppHandle) -> AppSettings {
    let path = get_settings_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&content) {
                return settings;
            }
        }
    }
    AppSettings::default()
}

/// 核心重构函数：将 AppSettings 中的全部可配置项（端口、TUN、路由等）统一同步到 config.json
pub fn rebuild_config_from_settings(app_handle: &tauri::AppHandle) -> Result<(), crate::error::AppError> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        return Ok(());
    }

    let settings = settings_get_internal(app_handle);

    // 读取当前 config.json
    let content = fs::read_to_string(&config_path)
        .map_err(|e| crate::error::AppError::Io(format!("读取 config.json 失败: {}", e)))?;
    let mut config_val: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| crate::error::AppError::Config(format!("解析 config.json 失败: {}", e)))?;

    let mut modified = false;

    // 1. 同步 Inbounds (包括端口 mixed_port 与开关 tun_enabled)
    if let Some(inbounds) = config_val.get_mut("inbounds").and_then(|i| i.as_array_mut()) {
        inbounds.clear();
        // 混合代理入口
        inbounds.push(serde_json::json!({
            "type": "mixed",
            "tag": "mixed-in",
            "listen": "127.0.0.1",
            "listen_port": settings.mixed_port
        }));
        
        // TUN 模式入口 (如果启用)
        if settings.tun_enabled {
            // 使用用户配置的网卡名称，默认 Auroweave
            let iface_name = if settings.tun_interface_name.trim().is_empty() {
                "Auroweave".to_string()
            } else {
                settings.tun_interface_name.clone()
            };
            inbounds.push(serde_json::json!({
                "type": "tun",
                "tag": "tun-in",
                "interface_name": iface_name,
                "address": ["172.19.0.1/30"],
                "auto_route": true,
                "strict_route": true,
                "stack": "system"
            }));
        }
        modified = true;
    }

    // 2. 同步 ClashAPI 端口
    if let Some(experimental) = config_val.get_mut("experimental").and_then(|e| e.as_object_mut()) {
        if let Some(clash_api) = experimental.get_mut("clash_api").and_then(|c| c.as_object_mut()) {
            clash_api.insert(
                "external_controller".to_string(),
                serde_json::json!(format!("127.0.0.1:{}", settings.clash_api_port))
            );
            modified = true;
        }
    }

    // 3. 移除 log.output 以便统一使用系统日志收集 stdout
    if let Some(log) = config_val.get_mut("log").and_then(|l| l.as_object_mut()) {
        if log.remove("output").is_some() {
            modified = true;
        }
    }

    if modified {
        let new_content = serde_json::to_string_pretty(&config_val)
            .map_err(|e| crate::error::AppError::Config(format!("序列化 config.json 失败: {}", e)))?;
        fs::write(&config_path, new_content)
            .map_err(|e| crate::error::AppError::Io(format!("写入 config.json 失败: {}", e)))?;
        log::info!("[settings] 已重建 config.json，已配置端口并包含 TUN 节点: {}", settings.tun_enabled);
    }

    Ok(())
}

/// 获取所有设置
#[tauri::command]
pub async fn settings_get_all(app_handle: tauri::AppHandle) -> ApiResponse<AppSettings> {
    ApiResponse::ok(settings_get_internal(&app_handle))
}

/// 内部保存设置辅助函数
pub fn update_settings_internal(app_handle: &tauri::AppHandle, patch: serde_json::Value) -> Result<(), String> {
    let path = get_settings_path();
    let mut current = if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            serde_json::from_str::<serde_json::Value>(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        }
    } else {
        serde_json::json!({})
    };

    if current.get("mixed_port").is_none() {
        if let Ok(default_val) = serde_json::to_value(AppSettings::default()) {
            current = default_val;
        }
    }

    if let Some(obj) = current.as_object_mut() {
        if let Some(patch_obj) = patch.as_object() {
            for (k, v) in patch_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
    }

    if let Ok(content) = serde_json::to_string_pretty(&current) {
        fs::write(&path, content).map_err(|e| e.to_string())?;
    }

    let _ = rebuild_config_from_settings(app_handle);
    Ok(())
}

/// 将当前的 config.json 配置同步下发给系统服务
pub async fn sync_config_to_service(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        return Err("config.json 配置文件不存在，请先导入订阅".to_string());
    }

    let config_path_str = config_path.to_string_lossy().to_string();
    log::info!("[settings] 向系统服务同步配置，路径: {}", config_path_str);
    
    let resp = crate::core::ipc_client::send_ipc_request("RELOAD_CONFIG", Some(&config_path_str)).await?;
    if !resp.success {
        return Err(resp.error.unwrap_or_else(|| "服务内部处理配置重载异常".to_string()));
    }
    Ok(())
}

/// 保存设置
///
/// 性能优化：仅当内核相关字段（mixed_port, clash_api_port, proxy_mode,
/// tun_enabled, run_mode）发生变化时才触发 apply_core_mode_with_fallback 重启内核，
/// 避免修改主题、性能模式等无关设置时产生不必要的内核重启。
#[tauri::command]
pub async fn settings_save(app_handle: tauri::AppHandle, patch: serde_json::Value) -> ApiResponse<()> {
    log::info!("[settings] 保存设置，补丁: {:?}", patch);

    // 记录保存前的设置，用于后续比较内核相关字段是否变化
    let old_settings = settings_get_internal(&app_handle);

    if let Err(e) = update_settings_internal(&app_handle, patch) {
        return ApiResponse::err(format!("写入设置失败: {}", e), 500);
    }

    // 读取保存后的设置
    let new_settings = settings_get_internal(&app_handle);

    // 判断内核相关字段是否变化
    let core_changed = old_settings.mixed_port != new_settings.mixed_port
        || old_settings.clash_api_port != new_settings.clash_api_port
        || old_settings.proxy_mode != new_settings.proxy_mode
        || old_settings.tun_enabled != new_settings.tun_enabled
        || old_settings.core.run_mode != new_settings.core.run_mode;

    if core_changed {
        log::info!("[settings] 检测到内核相关字段变化，触发配置重载和内核重启");
        // 统一通过自愈恢复逻辑应用配置和重载内核
        if let Err(e) = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
            return ApiResponse::err(e, 500);
        }
    } else {
        log::info!("[settings] 内核相关字段未变化，跳过内核重启");
    }

    ApiResponse::ok(())
}

/// 生成终端代理环境变量注入命令（开发者工具箱）
#[tauri::command]
pub async fn settings_inject_terminal_proxy(
    host: String,
    port: u16,
) -> ApiResponse<Vec<String>> {
    let proxy_url = format!("http://{}:{}", host, port);
    let commands = vec![
        format!("export http_proxy={}", proxy_url),
        format!("export https_proxy={}", proxy_url),
        format!("export all_proxy={}", proxy_url),
        format!("$env:http_proxy=\"{}\"", proxy_url),
        format!("$env:https_proxy=\"{}\"", proxy_url),
    ];
    ApiResponse::ok(commands)
}

/// 导出诊断日志（返回日志文件路径）
#[tauri::command]
pub async fn settings_export_diagnostic_log(app_handle: tauri::AppHandle) -> ApiResponse<String> {
    // 注意：日志文件由 lib.rs 中 tauri-plugin-log 写入到 get_log_dir() 目录
    // get_log_dir() = get_data_root()/logs，不能误用 get_config_dir()/logs
    let log_path = crate::get_log_dir().join("auroweave.log");
    
    if !log_path.exists() {
        return ApiResponse::err("诊断日志不存在，请先运行核心服务", 404);
    }
    
    match app_handle.path().desktop_dir() {
        Ok(desktop) => {
            let target_path = desktop.join("Auroweave_diagnostic_log.txt");
            match fs::copy(&log_path, &target_path) {
                Ok(_) => {
                    log::info!("[settings] 成功导出诊断日志至桌面: {:?}", target_path);
                    ApiResponse::ok(target_path.to_string_lossy().to_string())
                }
                Err(e) => ApiResponse::err(format!("复制日志到桌面失败: {}", e), 500),
            }
        }
        Err(e) => ApiResponse::err(format!("获取桌面路径失败: {}", e), 500),
    }
}

/// 专用 TUN 模式切换命令 (同步等待进程启动结果并将失败透传前端)
#[tauri::command]
pub async fn tun_set_enabled(app_handle: tauri::AppHandle, enabled: bool) -> ApiResponse<()> {
    log::info!("[settings] 切换 TUN 模式状态: enabled={}", enabled);

    let mut current_settings = settings_get_internal(&app_handle);
    current_settings.tun_enabled = enabled;
    
    if let Err(e) = update_settings_internal(&app_handle, serde_json::to_value(current_settings).unwrap()) {
        return ApiResponse::err(format!("保存设置失败: {}", e), 500);
    }

    // 统一通过自愈恢复逻辑应用配置和重载内核
    if let Err(e) = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
        return ApiResponse::err(e, 500);
    }
    
    ApiResponse::ok(())
}

/// 查询系统服务状态与版本
#[tauri::command]
pub async fn service_query_status(app_handle: tauri::AppHandle) -> ApiResponse<CoreServiceSettings> {
    let status = crate::system::service_control::query_service_status().unwrap_or_else(|e| {
        log::error!("[settings] 查询 Windows 系统服务状态异常: {}", e);
        "error".to_string()
    });

    let settings = settings_get_internal(&app_handle);
    let mut current_core = settings.core;
    current_core.service.last_known_status = status;

    ApiResponse::ok(current_core.service)
}

/// 提权安装系统服务或提权计划任务
#[tauri::command]
pub async fn service_install(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    let settings = settings_get_internal(&app_handle);
    let run_mode = settings.core.run_mode.clone();

    // 1. 定位当前有效 sing-box 路径以传给安装器
    let singbox_path = match crate::core::sidecar::SidecarManager::resolve_binary_path() {
        Ok(path) => path,
        Err(e) => {
            log::error!("[settings] 定位 sing-box 二进制失败: {:?}", e);
            return ApiResponse::err(format!("定位 sing-box 失败，请先下载: {}", e), 404);
        }
    };

    // 2. 调用 UAC 提权安装，传入 run_mode 和 sing-box 路径
    match crate::system::service_control::install_service_uac(&app_handle, &run_mode, &singbox_path) {
        Ok(_) => {
            let version = app_handle.package_info().version.to_string();
            let mut settings = settings_get_internal(&app_handle);
            settings.core.service.installed_version = Some(version);
            settings.core.service.last_fallback_reason = None;

            if run_mode == "service" {
                // 仅服务模式下自动启动服务
                let _ = crate::system::service_control::start_service();
                settings.core.service.last_known_status = "running".to_string();
                log::info!("[settings] 提权安装并启动系统服务成功");
            } else {
                // 本地运行模式下，不需要启动服务，更新状态为 stopped
                settings.core.service.last_known_status = "stopped".to_string();
                log::info!("[settings] 提权安装计划任务组件成功，本地运行模式无需启动 Windows 服务");
            }
            
            let _ = update_settings_internal(&app_handle, serde_json::to_value(settings).unwrap());
            ApiResponse::ok(())
        }
        Err(e) => {
            log::error!("[settings] 提权安装提权组件失败: {}", e);
            ApiResponse::err(format!("提权安装提权组件失败: {}", e), 500)
        }
    }
}

/// 提权卸载系统服务
#[tauri::command]
pub async fn service_uninstall(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    match crate::system::service_control::uninstall_service_uac(&app_handle) {
        Ok(_) => {
            let mut settings = settings_get_internal(&app_handle);
            settings.core.service.installed_version = None;
            settings.core.service.last_known_status = "not_installed".to_string();
            let _ = update_settings_internal(&app_handle, serde_json::to_value(settings).unwrap());
            log::info!("[settings] 提权卸载系统服务成功");
            ApiResponse::ok(())
        }
        Err(e) => {
            log::error!("[settings] 提权卸载系统服务失败: {}", e);
            ApiResponse::err(format!("提权卸载系统服务失败: {}", e), 500)
        }
    }
}

/// 手动启动系统服务
#[tauri::command]
pub async fn service_start(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    match crate::system::service_control::start_service() {
        Ok(_) => {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if let Err(e) = sync_config_to_service(&app_handle).await {
                return ApiResponse::err(format!("服务已成功拉起，但初始化内核配置失败: {}", e), 500);
            }
            log::info!("[settings] 手动拉起系统服务成功");
            ApiResponse::ok(())
        }
        Err(e) => {
            log::error!("[settings] 启动服务失败: {}", e);
            ApiResponse::err(format!("启动服务失败: {}", e), 500)
        }
    }
}

/// 手动停止系统服务
#[tauri::command]
pub async fn service_stop() -> ApiResponse<()> {
    match crate::system::service_control::stop_service() {
        Ok(_) => {
            log::info!("[settings] 手动停止系统服务成功");
            ApiResponse::ok(())
        }
        Err(e) => {
            log::error!("[settings] 停止服务失败: {}", e);
            ApiResponse::err(format!("停止服务失败: {}", e), 500)
        }
    }
}

/// 读取系统服务运行日志
#[tauri::command]
pub async fn service_read_log() -> ApiResponse<String> {
    // 守护进程 AuroDaemon 将日志写入 ProgramData/Auroweave/logs/service.log
    // 参见 crates/auroweave-svc/src/main.rs 的 init_file_logging()
    let log_path = crate::get_log_dir().join("service.log");
    if !log_path.exists() {
        return ApiResponse::err("系统服务运行日志文件不存在".to_string(), 404);
    }
    match fs::read_to_string(log_path) {
        Ok(c) => ApiResponse::ok(c),
        Err(e) => ApiResponse::err(format!("读取系统服务日志失败: {}", e), 500),
    }
}

/// 查询内核 sing-box 是否在真正运行中
#[tauri::command]
pub async fn core_query_running(app_handle: tauri::AppHandle) -> ApiResponse<bool> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        return ApiResponse::ok(false);
    }

    let settings = settings_get_internal(&app_handle);
    if settings.core.run_mode == "service" {
        match crate::core::ipc_client::send_ipc_request("GET_STATUS", None).await {
            Ok(resp) => {
                ApiResponse::ok(resp.success && (resp.status == "running" || resp.status == "starting"))
            }
            Err(_) => ApiResponse::ok(false)
        }
    } else {
        let sidecar_manager = app_handle.state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
        let status = sidecar_manager.get_status();
        let is_running = status == crate::core::sidecar::SidecarStatus::Running 
            || status == crate::core::sidecar::SidecarStatus::Starting;
        
        let is_tun_process_running = if settings.tun_enabled {
            // Windows: 通过计划任务检测；macOS/Linux: 检查 sidecar 状态即可
            #[cfg(target_os = "windows")]
            { crate::system::service_control::query_singbox_process_running() }
            #[cfg(not(target_os = "windows"))]
            { false }
        } else {
            false
        };
        
        ApiResponse::ok(is_running || is_tun_process_running)
    }
}

