/// IPC 命令 — 代理节点与分组
/// 作者: TanXiang
use crate::core::clash_api::ClashApiClient;
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyGroup {
    pub tag: String,
    pub r#type: String,
    pub proxies: Vec<String>,
    pub now: Option<String>,
    pub url: Option<String>,
    pub interval: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyNode {
    pub tag: String,
    pub r#type: String,
    pub region: Option<String>,
    pub country_code: Option<String>,
    pub is_active: Option<bool>,
}

/// 获取所有代理分组
#[tauri::command]
pub async fn proxy_get_groups() -> ApiResponse<Vec<ProxyGroup>> {
    let client = ClashApiClient::default();
    match client.get_proxies().await {
        Ok(json) => {
            let mut groups = Vec::new();
            if let Some(proxies) = json.get("proxies").and_then(|p| p.as_object()) {
                for (name, val) in proxies {
                    let group_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("Selector");
                    if group_type == "Selector" || group_type == "URLTest" {
                        let now = val.get("now").and_then(|n| n.as_str()).map(|s| s.to_string());
                        let list = val.get("all").and_then(|a| a.as_array())
                            .map(|arr| arr.iter().filter_map(|item| item.as_str().map(|s| s.to_string())).collect())
                            .unwrap_or_default();

                        groups.push(ProxyGroup {
                            tag: name.clone(),
                            r#type: group_type.to_lowercase(),
                            proxies: list,
                            now,
                            url: None,
                            interval: None,
                        });
                    }
                }
            }
            ApiResponse::ok(groups)
        }
        Err(e) => ApiResponse::err(e, 502),
    }
}

/// 获取分组内所有节点
#[tauri::command]
pub async fn proxy_get_group_nodes(group_tag: String) -> ApiResponse<Vec<ProxyNode>> {
    let client = ClashApiClient::default();
    match client.get_proxies().await {
        Ok(json) => {
            let mut nodes = Vec::new();
            if let Some(proxies) = json.get("proxies").and_then(|p| p.as_object()) {
                if let Some(group_val) = proxies.get(&group_tag) {
                    if let Some(all) = group_val.get("all").and_then(|a| a.as_array()) {
                        let current_now = group_val.get("now").and_then(|n| n.as_str()).unwrap_or("");
                        for item in all {
                            if let Some(node_name) = item.as_str() {
                                let node_type = proxies.get(node_name)
                                    .and_then(|n| n.get("type"))
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("unknown");

                                nodes.push(ProxyNode {
                                    tag: node_name.to_string(),
                                    r#type: node_type.to_string(),
                                    region: None,
                                    country_code: None,
                                    is_active: Some(node_name == current_now),
                                });
                            }
                        }
                    }
                }
            }
            ApiResponse::ok(nodes)
        }
        Err(e) => ApiResponse::err(e, 502),
    }
}

/// 切换分组当前节点
#[tauri::command]
pub async fn proxy_select_node(group_tag: String, node_tag: String) -> ApiResponse<()> {
    let client = ClashApiClient::default();
    match client.select_node(&group_tag, &node_tag).await {
        Ok(_) => ApiResponse::ok(()),
        Err(e) => ApiResponse::err(e, 500),
    }
}

/// 获取当前代理模式
#[tauri::command]
pub async fn proxy_get_mode() -> ApiResponse<String> {
    let client = ClashApiClient::default();
    match client.get_configs().await {
        Ok(configs) => {
            let mode = configs.get("mode")
                .and_then(|m| m.as_str())
                .unwrap_or("rule")
                .to_string();
            ApiResponse::ok(mode)
        }
        Err(e) => ApiResponse::err(e, 500),
    }
}

/// 切换代理模式
#[tauri::command]
pub async fn proxy_set_mode(app_handle: tauri::AppHandle, mode: String) -> ApiResponse<()> {
    if !["global", "rule", "direct"].contains(&mode.as_str()) {
        return ApiResponse::err(
            AppError::Validation(format!("无效的代理模式: {}", mode)),
            400,
        );
    }
    tracing::info!("切换代理模式: {}", mode);

    let mut settings = crate::commands::settings::settings_get_internal(&app_handle);
    settings.proxy_mode = mode.clone();
    
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("config"));
    let settings_path = config_dir.join("settings.json");
    if let Ok(content) = serde_json::to_string_pretty(&settings) {
        let _ = std::fs::write(&settings_path, content);
    }

    let client = ClashApiClient::default();
    let body = serde_json::json!({ "mode": mode });
    match client.patch_configs(body).await {
        Ok(_) => {
            if settings.tun_enabled {
                let _ = crate::system::sysproxy::set_system_proxy(false, settings.mixed_port);
            } else {
                let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
            }
            ApiResponse::ok(())
        }
        Err(e) => ApiResponse::err(e, 500),
    }
}

/// 强制设置 Windows 系统代理开启或注销 (供前端总开关与自救调用)
#[tauri::command]
pub async fn sysproxy_set(enabled: bool, port: u16) -> ApiResponse<()> {
    tracing::info!("强制设置系统代理状态: enabled={}, port={}", enabled, port);
    let _ = crate::system::sysproxy::set_system_proxy(enabled, port);
    ApiResponse::ok(())
}

/// 以管理员身份提权重启当前 Auroweave 程序
#[tauri::command]
pub async fn app_restart_as_admin(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    tracing::info!("准备以管理员身份提权重启程序");
    if let Ok(current_exe) = std::env::current_exe() {
        let exe_path = current_exe.to_string_lossy().to_string();
        
        #[cfg(target_os = "windows")]
        {
            // 通过 PowerShell 执行 Start-Process -Verb RunAs 提权运行当前 exe
            let status = std::process::Command::new("powershell")
                .args(&[
                    "-NoProfile",
                    "-WindowStyle", "Hidden",
                    "-Command",
                    &format!("Start-Process -FilePath '{}' -Verb RunAs", exe_path)
                ])
                .status();
            
            if status.is_ok() {
                // 注销系统代理，防止退出时残留
                let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                app_handle.exit(0);
                return ApiResponse::ok(());
            }
        }
    }
    ApiResponse::err("以管理员身份提权重启失败".to_string(), 500)
}
