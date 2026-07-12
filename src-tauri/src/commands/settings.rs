/// IPC 命令 — 应用设置
/// 作者: TanXiang
use crate::error::ApiResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub theme: String,
    pub language: String,
    pub proxy_mode: String,
    pub auto_start: bool,
    pub tun_enabled: bool,
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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "zh-CN".to_string(),
            proxy_mode: "rule".to_string(),
            auto_start: false,
            tun_enabled: false,
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
            inbounds.push(serde_json::json!({
                "type": "tun",
                "tag": "tun-in",
                "interface_name": "singbox-tun",
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

/// 保存设置
#[tauri::command]
pub async fn settings_save(app_handle: tauri::AppHandle, patch: serde_json::Value) -> ApiResponse<()> {
    let path = get_settings_path(&app_handle);
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
        if let Err(e) = fs::write(&path, content) {
            return ApiResponse::err(format!("写入设置文件失败: {}", e), 500);
        }
    }

    // 调用统一的 rebuild 函数更新 config.json
    let _ = rebuild_config_from_settings(&app_handle);

    // 重新拉起 sidecar 生效新设置
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
    let config_path = config_dir.join("config.json");
    if config_path.exists() {
        let sidecar_manager = app_handle.state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
        let config_path_str = config_path.to_string_lossy().to_string();
        let app_handle_clone = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let _ = crate::system::sysproxy::set_system_proxy(false, 0);
            let _ = sidecar_manager.stop().await;
            
            // 完全释放网络接管状态拦截：若是直连模式且 TUN 未启用，直接退出不自启动进程
            let settings = settings_get_internal(&app_handle_clone);
            if settings.proxy_mode == "direct" && !settings.tun_enabled {
                tracing::info!("系统处于直连且TUN关闭的完全释放状态，sing-box 进程保持停止且不自启动");
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
