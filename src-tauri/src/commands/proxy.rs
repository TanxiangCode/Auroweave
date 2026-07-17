/// IPC 命令 — 代理节点与分组
/// 作者: TanXiang
use crate::core::clash_api::ClashApiClient;
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};


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
///
/// 完整流程:
/// 1. 校验模式合法性 (global / rule / direct)
/// 2. 通过 update_settings_internal 统一保存 proxy_mode 到 settings.json（避免绕过统一逻辑）
/// 3. 重建 config.json 以反映新模式
/// 4. 若内核已运行 → ClashAPI patch 热切换模式 + 同步系统代理
/// 5. 若内核未运行且新模式需要接管 (rule/global) → apply_core_mode_with_fallback 拉起进程
/// 6. 若内核未运行且直连 + 未启用 TUN → 注销系统代理
#[tauri::command]
pub async fn proxy_set_mode(app_handle: tauri::AppHandle, mode: String) -> ApiResponse<()> {
    if !["global", "rule", "direct"].contains(&mode.as_str()) {
        return ApiResponse::err(
            AppError::Validation(format!("无效的代理模式: {}", mode)),
            400,
        );
    }
    log::info!("[proxy] 切换代理模式: {}", mode);

    // 统一通过 update_settings_internal 保存 proxy_mode，避免绕过设置保存逻辑
    let patch = serde_json::json!({ "proxy_mode": mode.clone() });
    if let Err(e) = crate::commands::settings::update_settings_internal(&app_handle, patch) {
        log::error!("[proxy] 保存 proxy_mode 失败: {}", e);
        return ApiResponse::err(format!("保存代理模式失败: {}", e), 500);
    }

    // 重新读取保存后的设置，用于后续判断
    let settings = crate::commands::settings::settings_get_internal(&app_handle);

    // 重新构建 config.json
    let _ = crate::commands::settings::rebuild_config_from_settings(&app_handle);

    let running_res = crate::commands::settings::core_query_running(app_handle.clone()).await;
    let is_running = running_res.data.unwrap_or(false);

    // 若进程在运行中，直接通过 Clash API 发送 patch，不重启进程
    if is_running {
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
    } else {
        // 核心判定：若需要接管 (rule/global) 且进程没跑，说明需要重新拉起后台核心
        if mode == "rule" || mode == "global" {
            log::info!("[proxy] sing-box 处于停止状态，开始重新拉起进程...");
            match crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
                Ok(_) => ApiResponse::ok(()),
                Err(e) => ApiResponse::err(e, 500),
            }
        } else {
            // 若核心停止运行，且处于直连且未启用 TUN (完全释放网络)，则注销 Windows 系统代理
            if mode == "direct" && !settings.tun_enabled {
                let _ = crate::system::sysproxy::set_system_proxy(false, 0);
            }
            ApiResponse::ok(())
        }
    }
}

/// 强制设置 Windows 系统代理开启或注销 (供前端总开关与自救调用)
#[tauri::command]
pub async fn sysproxy_set(enabled: bool, port: u16) -> ApiResponse<()> {
    log::info!("[proxy] 强制设置系统代理状态: enabled={}, port={}", enabled, port);
    let _ = crate::system::sysproxy::set_system_proxy(enabled, port);
    ApiResponse::ok(())
}

/// 以管理员身份提权重启当前 Auroweave 程序
#[tauri::command]
pub async fn app_restart_as_admin(app_handle: tauri::AppHandle) -> ApiResponse<()> {
log::info!("[proxy] 准备以管理员身份提权重启程序");
if let Ok(current_exe) = std::env::current_exe() {
let exe_path = current_exe.to_string_lossy().to_string();

#[cfg(target_os = "windows")]
{
// 通过 PowerShell 执行 Start-Process -Verb RunAs 提权运行当前 exe
// 安全防护：对 exe_path 中的单引号进行转义，防止 PowerShell 命令注入
// PowerShell 单引号字符串中，单引号用两个连续单引号表示
let escaped_path = exe_path.replace('\'', "''");
let ps_command = format!("Start-Process -FilePath '{}' -Verb RunAs", escaped_path);

let status = std::process::Command::new("powershell")
.args(&[
"-NoProfile",
"-WindowStyle", "Hidden",
"-Command",
&ps_command
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

/// 获取内核版本号
#[tauri::command]
pub async fn proxy_get_singbox_version() -> ApiResponse<String> {
    match crate::core::sidecar::SidecarManager::resolve_binary_path() {
        Ok(path) => {
            if let Ok(output) = std::process::Command::new(path).arg("version").output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // 提取版本号（sing-box version 1.14.0）
                for line in stdout.lines() {
                    if line.starts_with("sing-box version ") {
                        let version = line.replace("sing-box version ", "").trim().to_string();
                        return ApiResponse::ok(version);
                    }
                }
                ApiResponse::ok(stdout.lines().next().unwrap_or("Unknown").to_string())
            } else {
                ApiResponse::err("无法执行内核程序", 500)
            }
        }
        Err(_) => ApiResponse::err("未找到内核程序", 404),
    }
}
