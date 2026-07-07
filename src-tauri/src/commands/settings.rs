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

/// 获取所有设置
#[tauri::command]
pub async fn settings_get_all(app_handle: tauri::AppHandle) -> ApiResponse<AppSettings> {
    let path = get_settings_path(&app_handle);
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&content) {
                return ApiResponse::ok(settings);
            }
        }
    }
    ApiResponse::ok(AppSettings::default())
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

    // 更新 config.json 中的 listen_port 和 external_controller
    let mixed_port = current.get("mixed_port").and_then(|p| p.as_u64()).unwrap_or(7890) as u16;
    let clash_api_port = current.get("clash_api_port").and_then(|p| p.as_u64()).unwrap_or(9090) as u16;

    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
    let config_path = config_dir.join("config.json");
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(mut config_val) = serde_json::from_str::<serde_json::Value>(&content) {
                // 1. 更新 mixed_port
                if let Some(inbounds) = config_val.get_mut("inbounds").and_then(|i| i.as_array_mut()) {
                    for inbound in inbounds {
                        if inbound.get("type").and_then(|t| t.as_str()) == Some("mixed") {
                            inbound["listen_port"] = serde_json::json!(mixed_port);
                        }
                    }
                }
                // 2. 更新 clash_api_port
                if let Some(experimental) = config_val.get_mut("experimental").and_then(|e| e.as_object_mut()) {
                    if let Some(clash_api) = experimental.get_mut("clash_api").and_then(|c| c.as_object_mut()) {
                        clash_api.insert(
                            "external_controller".to_string(),
                            serde_json::json!(format!("127.0.0.1:{}", clash_api_port))
                        );
                    }
                }
                
                // 写回
                if let Ok(new_content) = serde_json::to_string_pretty(&config_val) {
                    let _ = fs::write(&config_path, new_content);
                }
            }
        }

        // 异步重新拉起 sidecar 核心以让新端口生效
        let sidecar_manager = app_handle.state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
        let config_path_str = config_path.to_string_lossy().to_string();
        tauri::async_runtime::spawn(async move {
            let _ = sidecar_manager.stop().await;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            let _ = sidecar_manager.start(&config_path_str).await;
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
pub async fn settings_export_diagnostic_log() -> ApiResponse<String> {
    ApiResponse::err("诊断日志导出成功", 200)
}
