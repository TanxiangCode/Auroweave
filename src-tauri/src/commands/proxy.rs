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
///
/// 完整流程:
/// 1. 校验模式合法性 (global / rule / direct)
/// 2. 通过 update_settings_internal 统一保存 proxy_mode 到 settings.json
/// 3. 重建 config.json（clash_api.default_mode 随之更新，rules 内置 direct 全量直连规则）
/// 4. 若内核已运行: PATCH /configs 运行时切换 clash mode（TUN/非 TUN 通用，
///    不重启进程、不重建 TUN 网卡，零弹窗平滑切换；PATCH 失败走核心自愈重启）
/// 5. 若内核未运行:
///    - rule/global → apply_core_mode_with_fallback 拉起进程
///    - direct + TUN → apply_core_mode_with_fallback 拉起 TUN 进程
///    - direct + 无 TUN → 直接启动 sing-box 子进程 + 设置系统代理
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

    // 若进程在运行中：无论是 TUN 模式还是非 TUN 模式，通过 Clash API 运行时热切换 mode
    // 无需重启 sing-box 进程，无需重建 TUN 网卡，实现零弹窗、毫秒级平滑切换。
    // 注意：PATCH /configs 的 mode 是唯一热切换真相源——route 规则以 clash_mode 条件
    // 联动（direct 时由 clash_mode:direct 规则全量直连），rebuild 不改写 route.final，
    // 保证"热切换"与"重启后"两种路径行为一致。
    if is_running {
        let client = ClashApiClient::default();
        let clash_mode = match mode.as_str() {
            "global" => "Global",
            "direct" => "Direct",
            _ => "Rule",
        };
        if let Err(e) = client.patch_configs(serde_json::json!({ "mode": clash_mode })).await {
            // sing-box 不存在配置热重载 API（PUT /configs 恒 204 空实现），
            // PATCH 失败说明内核异常（刚重启 API 未就绪/已崩溃）——走统一自愈重启
            log::warn!("[proxy] 通过 ClashAPI 热切换 mode 失败: {}，通过核心自愈流程恢复...", e);
            if let Err(re_err) = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
                return ApiResponse::err(format!("切换模式失败且核心恢复失败: {} / {}", e, re_err), 500);
            }
        }

        if settings.tun_enabled {
            let _ = crate::system::sysproxy::set_system_proxy(false, 0);
        } else {
            let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
        }
        log::info!("[proxy] 代理模式已热切换为: {}", mode);
        return ApiResponse::ok(());
    } else {
        // 核心未运行
        if mode == "rule" || mode == "global" {
            // rule/global 模式：通过 apply_core_mode_with_fallback 拉起进程
            log::info!("[proxy] sing-box 处于停止状态，开始重新拉起进程...");
            match crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
                Ok(_) => ApiResponse::ok(()),
                Err(e) => ApiResponse::err(e, 500),
            }
        } else if settings.tun_enabled {
            // direct 模式 + TUN 启用：通过 apply_core_mode_with_fallback 拉起 TUN 进程
            // (apply_core_mode_with_fallback 对 direct + TUN 不会提前返回，会正常启动)
            log::info!("[proxy] direct 模式 + TUN，sing-box 处于停止状态，开始拉起 TUN 进程...");
            match crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
                Ok(_) => ApiResponse::ok(()),
                Err(e) => ApiResponse::err(e, 500),
            }
        } else {
            // direct 模式 + 无 TUN：直接启动 sing-box 子进程 + 设置系统代理
            // 不调用 apply_core_mode_with_fallback（它对 direct+无TUN 会停止 sing-box）
            // 这样 direct 模式下流量仍走 sing-box（route.final=direct），可统计流量
            log::info!("[proxy] direct 模式 + 无 TUN，直接拉起 sing-box 用于流量统计...");
            let sm = app_handle.state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
            let config_dir = crate::get_config_dir();
            let config_path = config_dir.join("config.json");
            let config_path_str = config_path.to_string_lossy().to_string();
            match sm.start(&config_path_str).await {
                Ok(_) => {
                    let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
                    log::info!("[proxy] direct 模式 sing-box 已启动，系统代理已设置");
                    ApiResponse::ok(())
                }
                Err(e) => ApiResponse::err(format!("启动内核失败: {}", e), 500),
            }
        }
    }
}

/// 强制设置 Windows 系统代理开启或注销 (供前端总开关与自救调用)
#[tauri::command]
pub async fn sysproxy_set(enabled: bool, port: u16) -> ApiResponse<()> {
    log::info!("[proxy] 强制设置系统代理状态: enabled={}, port={}", enabled, port);
    match crate::system::sysproxy::set_system_proxy(enabled, port) {
        Ok(_) => ApiResponse::ok(()),
        Err(e) => {
            log::error!("[proxy] 设置系统代理失败: enabled={}, port={}, 原因: {}", enabled, port, e);
            ApiResponse::err(format!("设置系统代理失败: {}", e), 500)
        }
    }
}

/// 以管理员身份提权重启当前 Auroweave 程序
#[tauri::command]
pub async fn app_restart_as_admin(#[allow(unused_variables)] app_handle: tauri::AppHandle) -> ApiResponse<()> {
log::info!("[proxy] 准备以管理员身份提权重启程序");
if let Ok(current_exe) = std::env::current_exe() {
#[allow(unused_variables)]
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
///
/// Command::output() 是阻塞系统调用（外部进程执行 + 管道读取），
/// 移入 spawn_blocking 避免阻塞 tokio 异步运行时工作线程。
#[tauri::command]
pub async fn proxy_get_singbox_version() -> ApiResponse<String> {
    let version_result = tauri::async_runtime::spawn_blocking(|| {
        crate::core::sidecar::SidecarManager::resolve_binary_path().map(|path| {
            match std::process::Command::new(path).arg("version").output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    // 提取版本号（sing-box version 1.14.0）
                    for line in stdout.lines() {
                        if line.starts_with("sing-box version ") {
                            return line.replace("sing-box version ", "").trim().to_string();
                        }
                    }
                    stdout.lines().next().unwrap_or("Unknown").to_string()
                }
                Err(_) => "Unknown".to_string(),
            }
        })
    })
    .await
    .unwrap_or_else(|e| Err(crate::error::AppError::Unknown(format!("版本查询任务失败: {}", e))));

    match version_result {
        Ok(version) => ApiResponse::ok(version),
        Err(_) => ApiResponse::err("未找到内核程序", 404),
    }
}

/// 关闭单条活跃连接
#[tauri::command]
pub async fn proxy_close_connection(id: String) -> ApiResponse<()> {
    let client = ClashApiClient::default();
    match client.close_connection(&id).await {
        Ok(_) => ApiResponse::ok(()),
        Err(e) => ApiResponse::err(e.to_string(), 500),
    }
}

/// 关闭所有活跃连接
#[tauri::command]
pub async fn proxy_close_all_connections() -> ApiResponse<()> {
    let client = ClashApiClient::default();
    match client.close_all_connections().await {
        Ok(_) => ApiResponse::ok(()),
        Err(e) => ApiResponse::err(e.to_string(), 500),
    }
}

