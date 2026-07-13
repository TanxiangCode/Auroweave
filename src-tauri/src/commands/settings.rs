/// IPC 命令 — 应用设置
/// 作者: TanXiang
use crate::error::ApiResponse;
use serde::{Deserialize, Serialize};

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
        run_mode: "direct".to_string(),
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
            mixed_port: 7890,
            clash_api_port: 9090,
            speed_test_url: "https://speed.cloudflare.com/__down?bytes=25000000".to_string(),
            speed_test_timeout_secs: 5,
            connection_timeout_secs: 15,
            core: default_core_settings(),
        }
    }
}

use tauri::Manager;
use std::fs;
use std::path::PathBuf;

fn get_settings_path(app_handle: &tauri::AppHandle) -> PathBuf {
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
    let _ = fs::create_dir_all(&config_dir);
    config_dir.join("settings.json")
}

/// 内部加载设置函数 (供各 Rust 模块使用)
pub fn settings_get_internal(app_handle: &tauri::AppHandle) -> AppSettings {
    let path = get_settings_path(app_handle);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&content) {
                return settings;
            }
        }
    }
    AppSettings::default()
}

/// 核心重构函数：将 AppSettings 中的全部可配置项（端口、TUN、路由等）统一同步到 config.json
pub fn rebuild_config_from_settings(app_handle: &tauri::AppHandle) -> Result<(), crate::error::AppError> {
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
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

    // 3. 保证 log.output 也是正确的绝对路径
    let log_path_str = config_dir.join("box.log").to_string_lossy().to_string();
    if let Some(log) = config_val.get_mut("log").and_then(|l| l.as_object_mut()) {
        log.insert("output".to_string(), serde_json::json!(log_path_str));
        modified = true;
    }

    if modified {
        let new_content = serde_json::to_string_pretty(&config_val)
            .map_err(|e| crate::error::AppError::Config(format!("序列化 config.json 失败: {}", e)))?;
        fs::write(&config_path, new_content)
            .map_err(|e| crate::error::AppError::Io(format!("写入 config.json 失败: {}", e)))?;
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
    let path = get_settings_path(app_handle);
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
pub async fn sync_config_to_service(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        return Err("config.json 配置文件不存在，请先导入订阅".to_string());
    }

    let config_path_str = config_path.to_string_lossy().to_string();
    let resp = crate::core::ipc_client::send_ipc_request("RELOAD_CONFIG", Some(&config_path_str)).await?;
    if !resp.success {
        return Err(resp.error.unwrap_or_else(|| "服务内部处理配置重载异常".to_string()));
    }
    Ok(())
}

/// 保存设置
#[tauri::command]
pub async fn settings_save(app_handle: tauri::AppHandle, patch: serde_json::Value) -> ApiResponse<()> {
    let old_settings = settings_get_internal(&app_handle);
    if let Err(e) = update_settings_internal(&app_handle, patch) {
        return ApiResponse::err(format!("写入设置失败: {}", e), 500);
    }
    let new_settings = settings_get_internal(&app_handle);

    // 如果模式发生了切换
    if old_settings.core.run_mode != new_settings.core.run_mode {
        if new_settings.core.run_mode == "service" {
            // 从 direct 切换到 service
            let sidecar_manager = app_handle.state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
            let _ = sidecar_manager.stop().await;
            let _ = crate::system::sysproxy::set_system_proxy(false, 0);

            // 检查并确保服务处于运行状态
            let status = crate::system::service_control::query_service_status().unwrap_or_default();
            if status == "stopped" {
                if let Err(e) = crate::system::service_control::start_service() {
                    return ApiResponse::err(format!("启动系统服务失败: {}", e), 500);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if let Err(e) = sync_config_to_service(&app_handle).await {
                return ApiResponse::err(format!("同步配置至服务失败: {}", e), 500);
            }

            if new_settings.tun_enabled {
                let _ = crate::system::sysproxy::set_system_proxy(false, 0);
            } else {
                let _ = crate::system::sysproxy::set_system_proxy(true, new_settings.mixed_port);
            }
        } else {
            // 从 service 切换到 direct
            let _ = crate::system::service_control::stop_service();
            let _ = crate::system::sysproxy::set_system_proxy(false, 0);

            let sidecar_manager = app_handle.state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
            let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
            let config_path = config_dir.join("config.json");
            if config_path.exists() {
                let config_path_str = config_path.to_string_lossy().to_string();
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                if sidecar_manager.start(&config_path_str).await.is_ok() {
                    if new_settings.tun_enabled {
                        let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                    } else {
                        let _ = crate::system::sysproxy::set_system_proxy(true, new_settings.mixed_port);
                    }
                }
            }
        }
    } else {
        // 模式未变，只是更新设置参数
        if new_settings.core.run_mode == "service" {
            // 同步配置
            let _ = sync_config_to_service(&app_handle).await;
            if new_settings.proxy_mode == "direct" && !new_settings.tun_enabled {
                let _ = crate::core::ipc_client::send_ipc_request("SHUTDOWN_CORE", None).await;
                let _ = crate::system::sysproxy::set_system_proxy(false, 0);
            } else {
                if new_settings.tun_enabled {
                    let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                } else {
                    let _ = crate::system::sysproxy::set_system_proxy(true, new_settings.mixed_port);
                }
            }
        } else {
            // 直接运行模式下的重载逻辑
            let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
            let config_path = config_dir.join("config.json");
            if config_path.exists() {
                let sidecar_manager = app_handle.state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
                let config_path_str = config_path.to_string_lossy().to_string();
                let app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                    let _ = sidecar_manager.stop().await;

                    let settings = settings_get_internal(&app_handle_clone);
                    if settings.proxy_mode == "direct" && !settings.tun_enabled {
                        tracing::info!("系统处于直连且TUN关闭，sing-box 保持停止");
                        return;
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    if sidecar_manager.start(&config_path_str).await.is_ok() {
                        if settings.tun_enabled {
                            let _ = crate::system::sysproxy::set_system_proxy(false, settings.mixed_port);
                        } else {
                            let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
                        }
                    }
                });
            }
        }
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
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
    let log_path = config_dir.join("box.log");
    
    if !log_path.exists() {
        return ApiResponse::err("诊断日志不存在，请先运行核心服务", 404);
    }
    
    match app_handle.path().desktop_dir() {
        Ok(desktop) => {
            let target_path = desktop.join("Auroweave_diagnostic_log.txt");
            match fs::copy(&log_path, &target_path) {
                Ok(_) => ApiResponse::ok(target_path.to_string_lossy().to_string()),
                Err(e) => ApiResponse::err(format!("复制日志到桌面失败: {}", e), 500),
            }
        }
        Err(e) => ApiResponse::err(format!("获取桌面路径失败: {}", e), 500),
    }
}

/// 专用 TUN 模式切换命令 (同步等待进程启动结果并将失败透传前端)
#[tauri::command]
pub async fn tun_set_enabled(app_handle: tauri::AppHandle, enabled: bool) -> ApiResponse<()> {
    tracing::info!("设置 TUN 接管模式: enabled={}", enabled);

    let mut current_settings = settings_get_internal(&app_handle);
    current_settings.tun_enabled = enabled;
    
    if let Err(e) = update_settings_internal(&app_handle, serde_json::to_value(current_settings).unwrap()) {
        return ApiResponse::err(format!("保存设置失败: {}", e), 500);
    }

    let settings = settings_get_internal(&app_handle);
    let sidecar_manager = app_handle.state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
    
    // 清理系统代理
    let _ = crate::system::sysproxy::set_system_proxy(false, 0);

    if settings.core.run_mode == "service" {
        // 服务运行模式
        if !enabled {
            if let Err(e) = sync_config_to_service(&app_handle).await {
                return ApiResponse::err(format!("重载服务配置失败: {}", e), 500);
            }
            tracing::info!("系统服务 TUN 模式已关闭");
            return ApiResponse::ok(());
        }

        // 开启 TUN
        let status = crate::system::service_control::query_service_status().unwrap_or_default();
        if status != "running" {
            if let Err(e) = crate::system::service_control::start_service() {
                return ApiResponse::err(format!("启动系统服务失败: {}", e), 500);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        match sync_config_to_service(&app_handle).await {
            Ok(_) => {
                tracing::info!("系统服务 TUN 模式启动成功");
                ApiResponse::ok(())
            }
            Err(e) => {
                tracing::error!("系统服务 TUN 模式启动失败: {:?}", e);
                // 失败时回滚 settings
                let mut fallback_settings = settings_get_internal(&app_handle);
                fallback_settings.tun_enabled = false;
                let _ = update_settings_internal(&app_handle, serde_json::to_value(fallback_settings).unwrap());
                ApiResponse::err(format!("启动系统服务 TUN 失败: {}", e), 403)
            }
        }
    } else {
        // 直接运行模式
        let _ = sidecar_manager.stop().await;

        if !enabled {
            let _ = crate::system::service_control::stop_direct_tun_task();
            tracing::info!("直连模式下 TUN 已关闭");
            return ApiResponse::ok(());
        }

        // 停止之前的直连任务以防冲突
        let _ = crate::system::service_control::stop_direct_tun_task();

        let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
        let config_path = config_dir.join("config.json");
        if !config_path.exists() {
            return ApiResponse::err("config.json 不存在，请先导入订阅", 404);
        }

        // 复制配置文件到计划任务可读取的服务公共目录 %ProgramData%\Auroweave\config.json
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        let cache_dir = PathBuf::from(program_data).join("Auroweave");
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            return ApiResponse::err(format!("创建服务公共缓存目录失败: {}", e), 500);
        }
        let cache_config_path = cache_dir.join("config.json");
        if let Err(e) = fs::copy(&config_path, &cache_config_path) {
            return ApiResponse::err(format!("同步配置文件至服务缓存目录失败: {}", e), 500);
        }

        // 通过运行计划任务来静默提权启动内核
        if let Err(e) = crate::system::service_control::run_direct_tun_task() {
            let mut fallback_settings = settings_get_internal(&app_handle);
            fallback_settings.tun_enabled = false;
            let _ = update_settings_internal(&app_handle, serde_json::to_value(fallback_settings).unwrap());
            return ApiResponse::err(format!("静默提权启动 TUN 失败: {}", e), 403);
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
        match crate::system::service_control::query_direct_tun_task_running() {
            Ok(true) => {
                tracing::info!("直连模式下通过计划任务静默提权启动 TUN 成功");
                ApiResponse::ok(())
            }
            _ => {
                tracing::error!("直连模式下通过计划任务静默提权启动 TUN 失败: 计划任务未处于运行中");
                let mut fallback_settings = settings_get_internal(&app_handle);
                fallback_settings.tun_enabled = false;
                let _ = update_settings_internal(&app_handle, serde_json::to_value(fallback_settings).unwrap());
                ApiResponse::err("静默启动 TUN 任务失败，请确认是否已成功安装服务".to_string(), 403)
            }
        }
    }
}

/// 查询系统服务状态与版本
#[tauri::command]
pub async fn service_query_status(app_handle: tauri::AppHandle) -> ApiResponse<CoreServiceSettings> {
    let status = crate::system::service_control::query_service_status().unwrap_or_else(|e| {
        tracing::error!("查询 Windows 系统服务状态异常: {}", e);
        "error".to_string()
    });

    let settings = settings_get_internal(&app_handle);
    let mut current_core = settings.core;
    current_core.service.last_known_status = status;

    ApiResponse::ok(current_core.service)
}

/// 提权安装系统服务
#[tauri::command]
pub async fn service_install(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    match crate::system::service_control::install_service_uac(&app_handle) {
        Ok(_) => {
            // 安装成功后，自动为用户启动该服务
            let _ = crate::system::service_control::start_service();
            let version = app_handle.package_info().version.to_string();
            
            let mut settings = settings_get_internal(&app_handle);
            settings.core.service.installed_version = Some(version);
            settings.core.service.last_known_status = "running".to_string();
            settings.core.service.last_fallback_reason = None;
            let _ = update_settings_internal(&app_handle, serde_json::to_value(settings).unwrap());
            
            ApiResponse::ok(())
        }
        Err(e) => ApiResponse::err(format!("提权安装系统服务失败: {}", e), 500),
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
            ApiResponse::ok(())
        }
        Err(e) => ApiResponse::err(format!("提权卸载系统服务失败: {}", e), 500),
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
            ApiResponse::ok(())
        }
        Err(e) => ApiResponse::err(format!("启动服务失败: {}", e), 500),
    }
}

/// 手动停止系统服务
#[tauri::command]
pub async fn service_stop() -> ApiResponse<()> {
    match crate::system::service_control::stop_service() {
        Ok(_) => ApiResponse::ok(()),
        Err(e) => ApiResponse::err(format!("停止服务失败: {}", e), 500),
    }
}

/// 读取系统服务运行日志
#[tauri::command]
pub async fn service_read_log() -> ApiResponse<String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let log_path = std::path::PathBuf::from(program_data).join("Auroweave").join("service.log");
    if !log_path.exists() {
        return ApiResponse::err("系统服务运行日志文件不存在".to_string(), 404);
    }
    match fs::read_to_string(log_path) {
        Ok(c) => ApiResponse::ok(c),
        Err(e) => ApiResponse::err(format!("读取系统服务日志失败: {}", e), 500),
    }
}
