/// IPC 命令 — 订阅管理
/// 作者: TanXiang
///
/// 订阅信息持久化到 config_dir/subscriptions.json
/// 流程: 导入 → 拉取 → 解析 → 生成config.json → 拉起/热重载 → 持久化订阅元数据
use crate::core::clash_api::ClashApiClient;
use crate::core::config_builder::ConfigBuilder;
use crate::core::parser::{parse_subscription_content, SubscriptionFormat};
use crate::core::sidecar::SidecarManager;
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubscriptionUserInfo {
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
    pub expire_timestamp: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubscriptionFilterRule {
    pub include_pattern: Option<String>,
    pub exclude_pattern: Option<String>,
    pub rename_pattern: Option<String>,
    pub rename_replace: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub format: String,
    #[serde(default = "default_source_type")]
    pub source_type: String, // "remote" | "local_file" | "clipboard"
    pub local_file_path: Option<String>,
    pub user_agent: Option<String>,
    pub auto_update_interval_hours: Option<u32>,
    pub last_updated: Option<i64>,
    pub node_count: Option<u32>,
    #[serde(default)]
    pub is_active: bool,
    pub user_info: Option<SubscriptionUserInfo>,
    pub filter_rule: Option<SubscriptionFilterRule>,
}

fn default_source_type() -> String {
    "remote".to_string()
}


/// 获取订阅持久化文件路径
fn get_subscriptions_path() -> std::path::PathBuf {
    crate::get_config_dir().join("subscriptions.json")
}

/// 从磁盘读取所有已保存的订阅
fn load_subscriptions() -> Vec<Subscription> {
    let path = get_subscriptions_path();
    if !path.exists() {
        return Vec::new();
    }
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// 将订阅列表写入磁盘
fn save_subscriptions(subs: &[Subscription]) -> Result<(), AppError> {
    let path = get_subscriptions_path();
    let json = serde_json::to_string_pretty(subs)
        .map_err(|e| AppError::Io(format!("序列化订阅列表失败: {}", e)))?;
    fs::write(&path, json)
        .map_err(|e| AppError::Io(format!("写入订阅列表失败: {}", e)))?;
    Ok(())
}


/// 解析 HTTP 响应头中的 Subscription-Userinfo
/// 格式示例: upload=1073741824; download=10737418240; total=107374182400; expire=1735689600
fn parse_subscription_userinfo(header_val: &str) -> Option<SubscriptionUserInfo> {
    let mut upload = 0u64;
    let mut download = 0u64;
    let mut total = 0u64;
    let mut expire = None;

    for part in header_val.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            let k = k.trim().to_lowercase();
            let v = v.trim();
            match k.as_str() {
                "upload" => upload = v.parse().unwrap_or(0),
                "download" => download = v.parse().unwrap_or(0),
                "total" => total = v.parse().unwrap_or(0),
                "expire" => expire = v.parse().ok(),
                _ => {}
            }
        }
    }

    if total > 0 || upload > 0 || download > 0 || expire.is_some() {
        Some(SubscriptionUserInfo {
            upload_bytes: upload,
            download_bytes: download,
            total_bytes: total,
            expire_timestamp: expire,
        })
    } else {
        None
    }
}

/// 获取原始订阅数据文件路径
fn get_raw_subscription_path(id: &str) -> std::path::PathBuf {
    let dir = crate::get_config_dir().join("subscriptions_raw");
    let _ = fs::create_dir_all(&dir);
    dir.join(format!("{}.txt", id))
}

/// 保存原始订阅文本
fn save_raw_subscription(id: &str, content: &str) {
    let path = get_raw_subscription_path(id);
    let _ = fs::write(&path, content);
}

/// 读取已持久化的原始订阅文本
fn load_raw_subscription(id: &str) -> Option<String> {
    let path = get_raw_subscription_path(id);
    fs::read_to_string(&path).ok()
}

pub const DEFAULT_SUBSCRIPTION_UA: &str = "ClashMeta;sing-box;Auroweave/1.0";

/// 内部核心函数：拉取订阅 URL 内容并解析为 outbounds 与 userinfo
async fn fetch_and_parse(url: &str, custom_ua: Option<&str>) -> Result<(SubscriptionFormat, Vec<crate::core::parser::ParsedOutbound>, Option<SubscriptionUserInfo>, String), AppError> {
    let ua = custom_ua.unwrap_or(DEFAULT_SUBSCRIPTION_UA);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(ua)
        .build()
        .map_err(|e| AppError::Network(format!("创建网络客户端失败: {}", e)))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("拉取订阅失败: {}", e)))?;

    let user_info = resp
        .headers()
        .get("subscription-userinfo")
        .or_else(|| resp.headers().get("Subscription-Userinfo"))
        .and_then(|h| h.to_str().ok())
        .and_then(parse_subscription_userinfo);

    let content = resp
        .text()
        .await
        .map_err(|e| AppError::Network(format!("读取订阅响应失败: {}", e)))?;

    let (format, outbounds) = parse_subscription_content(&content)?;
    Ok((format, outbounds, user_info, content))
}


/// 下载 rule-set .srs 文件到本地缓存目录
///
/// 如果文件已存在且大小 > 0，直接返回路径（跳过下载）。
/// 否则尝试直连下载（不经代理），失败则返回 None（配置降级为无 rule-set）。
async fn download_rule_set(config_dir: &std::path::Path, name: &str, url: &str) -> Option<String> {
    let local_path = config_dir.join(format!("{}.srs", name));

    if local_path.exists() {
        if let Ok(meta) = std::fs::metadata(&local_path) {
            if meta.len() > 0 {
                return Some(local_path.to_string_lossy().to_string());
            }
        }
    }

    log::info!("[subscription] 下载 rule-set: {} from {}", name, url);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;

    match client.get(url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                log::warn!("[subscription] 下载 {} 失败: HTTP {}", name, resp.status());
                return None;
            }
            match resp.bytes().await {
                Ok(bytes) if !bytes.is_empty() => {
                    if let Err(e) = std::fs::write(&local_path, &bytes) {
                        log::warn!("[subscription] 写入 {} 文件失败: {}", name, e);
                        return None;
                    }
                    log::info!("[subscription] rule-set {} 下载成功 ({} bytes)", name, bytes.len());
                    Some(local_path.to_string_lossy().to_string())
                }
                _ => {
                    log::warn!("[subscription] 下载 {} 返回空内容", name);
                    None
                }
            }
        }
        Err(e) => {
            log::warn!("[subscription] 下载 {} 网络错误: {}（将使用无 rule-set 降级配置）", name, e);
            None
        }
    }
}

/// 内部核心函数：根据 outbounds 生成 config.json 并热重载/拉起 sing-box
async fn build_and_apply_config(
    app_handle: &AppHandle,
    outbounds: Vec<crate::core::parser::ParsedOutbound>,
    _sidecar_manager: &SidecarManager,
) -> Result<u32, AppError> {
    if outbounds.is_empty() {
        return Err(AppError::Config("解析出的节点为空，无法生成配置".to_string()));
    }

    let node_count = outbounds.len() as u32;

    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);

    // 尝试下载 rule-set .srs 文件到本地（直连，不经代理）
    let geosite_cn_path = download_rule_set(
        &config_dir,
        "geosite-cn",
        "https://fastly.jsdelivr.net/gh/SagerNet/sing-geosite@rule-set/geosite-cn.srs",
    ).await;
    let geoip_cn_path = download_rule_set(
        &config_dir,
        "geoip-cn",
        "https://fastly.jsdelivr.net/gh/SagerNet/sing-geoip@rule-set/geoip-cn.srs",
    ).await;

    let settings = crate::commands::settings::settings_get_internal(app_handle);
    let (mixed_port, clash_api_port) = crate::speedtest::get_configured_ports(app_handle);
    let config_builder = ConfigBuilder::new(outbounds)
        .with_ports(mixed_port, clash_api_port)
        .with_allow_lan(settings.allow_lan)
        .with_local_rule_sets(geosite_cn_path, geoip_cn_path);
    let config_json = config_builder.build()?;


    let config_path = config_dir.join("config.json");
    let config_path_str = config_path.to_string_lossy().to_string();
    let backup_path = config_dir.join("config.backup.json");

    // 备份当前配置文件
    if config_path.exists() {
        let _ = fs::copy(&config_path, &backup_path);
    }

    fs::write(
        &config_path,
        serde_json::to_string_pretty(&config_json).unwrap_or_default(),
    )
    .map_err(|e| AppError::Io(format!("保存 config.json 失败: {}", e)))?;

    // 应用 TUN / 端口等设置覆写
    let _ = crate::commands::settings::rebuild_config_from_settings(app_handle);

    // 热重载或拉起 sing-box
    let clash_client = ClashApiClient::default();
    let reload_result = clash_client.reload_config(&config_path_str).await;
    let reload_success = reload_result.is_ok();

    if !reload_success {
        let err_msg = reload_result.err().map(|e| e.to_string()).unwrap_or_default();
        log::info!("[subscription] ClashAPI 热重载未生效 ({})，通过统一核心自愈恢复流程拉起内核...", err_msg);
        
        if let Err(e) = crate::system::startup::apply_core_mode_with_fallback(app_handle).await {
            log::error!("[subscription] 拉起 sing-box 失败 ({})，尝试自动回滚备份...", e);
            if backup_path.exists() {
                let _ = fs::copy(&backup_path, &config_path);
                let _ = crate::system::startup::apply_core_mode_with_fallback(app_handle).await;
            }
            return Err(AppError::Sidecar(format!(
                "启动核心失败，已自动回滚备份: {}",
                e
            )));
        }
    } else {
        log::info!("[subscription] sing-box 已成功通过 ClashAPI 热重载配置");
    }

    // 等待 ClashAPI 完全就绪（热重载后需要短暂时间让新配置生效）
    let mut api_ready = false;
    for i in 0..10 {
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        if clash_client.get_proxies().await.is_ok() {
            api_ready = true;
            log::info!("[subscription] ClashAPI 已就绪 (等待 {}ms)", (i + 1) * 300);
            break;
        }
    }
    if !api_ready {
        log::warn!("[subscription] ClashAPI 在 3 秒内未就绪，后续代理列表可能暂时为空");
    }

    // 关键修复：配置重载后自动触发 URLTest 组测速，使其能选择最优节点
    // 避免用户看不到 auto 组当前使用的节点
    if api_ready {
        let urltest_groups = match clash_client.get_proxies().await {
            Ok(json) => {
                if let Some(proxies) = json.get("proxies").and_then(|p| p.as_object()) {
                    proxies.iter()
                        .filter(|(_, v)| {
                            v.get("type").and_then(|t| t.as_str()) == Some("URLTest")
                                && !v.get("now").and_then(|n| n.as_str()).map(|s| !s.is_empty()).unwrap_or(false)
                        })
                        .map(|(name, _)| name.clone())
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            }
            Err(e) => {
                log::warn!("[subscription] 获取 URLTest 组失败: {}", e);
                Vec::new()
            }
        };

        if !urltest_groups.is_empty() {
            let test_count = urltest_groups.len();
            log::info!("[subscription] 检测到 {} 个未选择节点的 URLTest 组，开始自动测速", test_count);
            
            for group_tag in urltest_groups {
                log::info!("[subscription] 触发 URLTest 组 {} 的自动测速", group_tag);
                // 调用新添加的 trigger_urltest_group_delay 方法
                let delay_url = "http://www.gstatic.com/generate_204";
                let _ = tokio::time::timeout(
                    tokio::time::Duration::from_secs(10),
                    clash_client.trigger_urltest_group_delay(&group_tag, delay_url, 5000)
                ).await;


                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }

            log::info!("[subscription] 已触发 {} 个 URLTest 组的自动测速，使其选择最优节点", test_count);
        }
    }

    // 同步系统代理状态
    let settings = crate::commands::settings::settings_get_internal(app_handle);
    if settings.proxy_mode != "direct" {
        let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
    } else {
        let _ = crate::system::sysproxy::set_system_proxy(false, settings.mixed_port);
    }

    Ok(node_count)
}

/// 导入订阅并生成/热重载/拉起 sing-box，同时将其设为活跃订阅
#[tauri::command]
pub async fn subscription_import(
    app_handle: AppHandle,
    name: String,
    url: String,
    _auto_group: bool,
    sidecar_manager: State<'_, Arc<SidecarManager>>,
) -> Result<ApiResponse<Subscription>, AppError> {
    log::info!("[subscription] 开始导入订阅: {} (URL: {})", name, url);

    // 0. 重复检测：如果 URL 已存在则提示覆盖
    let existing = load_subscriptions();
    if let Some(existing_sub) = existing.iter().find(|s| s.url == url) {
        log::warn!("[subscription] 订阅 URL 已存在: {} (ID: {})", existing_sub.name, existing_sub.id);
        return Ok(ApiResponse::ok(existing_sub.clone()));
    }

    // 1+2. 拉取并解析
    let (format, outbounds, user_info, raw_text) = match fetch_and_parse(&url, None).await {
        Ok(res) => res,
        Err(e) => {
            log::error!("[subscription] 拉取/解析订阅失败: {}", e);
            return Ok(ApiResponse::err(e, 400));
        }
    };

    let format_str = match format {
        SubscriptionFormat::SingboxJson => "singbox",
        SubscriptionFormat::ClashYaml => "clash",
        SubscriptionFormat::Base64Uri => "v2ray",
        SubscriptionFormat::Unknown => "unknown",
    }
    .to_string();

    log::info!(
        "[subscription] 订阅解析成功，格式: {}, 节点数: {}",
        format_str,
        outbounds.len()
    );

    // 3+4+5. 生成配置并应用
    let node_count = match build_and_apply_config(&app_handle, outbounds, sidecar_manager.inner()).await {
        Ok(n) => n,
        Err(e) => {
            log::error!("[subscription] 应用配置失败: {}", e);
            return Ok(ApiResponse::err(e, 500));
        }
    };

    // 6. 持久化：复用已加载的订阅列表，将所有旧订阅设为非活跃，新订阅设为活跃
    let mut all_subs = existing;
    for s in all_subs.iter_mut() {
        s.is_active = false;
    }

    let sub = Subscription {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
        format: format_str,
        source_type: "remote".to_string(),
        local_file_path: None,
        user_agent: None,
        auto_update_interval_hours: Some(12),
        last_updated: Some(chrono::Utc::now().timestamp_millis()),
        node_count: Some(node_count),
        is_active: true,
        user_info,
        filter_rule: None,
    };

    save_raw_subscription(&sub.id, &raw_text);
    all_subs.push(sub.clone());
    if let Err(e) = save_subscriptions(&all_subs) {
        log::warn!("[subscription] 持久化订阅列表失败: {}", e);
    }

    Ok(ApiResponse::ok(sub))
}

/// 获取所有已保存订阅（从 subscriptions.json 读取）
#[tauri::command]
pub async fn subscription_get_all() -> ApiResponse<Vec<Subscription>> {
    ApiResponse::ok(load_subscriptions())
}

/// 删除订阅（从 subscriptions.json 中移除对应记录）
#[tauri::command]
pub async fn subscription_delete(id: String) -> ApiResponse<()> {
    log::info!("[subscription] 删除订阅: {}", id);
    let mut all_subs = load_subscriptions();
    let before_len = all_subs.len();
    all_subs.retain(|s| s.id != id);
    if all_subs.len() < before_len {
        let _ = fs::remove_file(get_raw_subscription_path(&id));
        if let Err(e) = save_subscriptions(&all_subs) {
            return ApiResponse::err(format!("删除订阅失败: {}", e), 500);
        }
        log::info!("[subscription] 订阅 {} 已删除", id);
    }
    ApiResponse::ok(())
}

/// 批量删除所有订阅（清空订阅列表）
#[tauri::command]
pub async fn subscription_delete_all() -> ApiResponse<()> {
    log::info!("[subscription] 删除所有订阅");

    let raw_dir = crate::get_config_dir().join("subscriptions_raw");
    let _ = fs::remove_dir_all(&raw_dir);

    if !get_subscriptions_path().exists() {
        return ApiResponse::ok(());
    }

    if let Err(e) = fs::remove_file(get_subscriptions_path()) {
        return ApiResponse::err(format!("清空订阅列表失败: {}", e), 500);
    }

    log::info!("[subscription] 所有订阅已清空");
    ApiResponse::ok(())
}

/// 刷新订阅（重新拉取 URL 并更新节点）
#[tauri::command]
pub async fn subscription_refresh(
    app_handle: AppHandle,
    id: String,
    sidecar_manager: State<'_, Arc<SidecarManager>>,
) -> Result<ApiResponse<Subscription>, AppError> {
    log::info!("[subscription] 刷新订阅: {}", id);

    let mut all_subs = load_subscriptions();
    let sub = match all_subs.iter().find(|s| s.id == id) {
        Some(s) => s.clone(),
        None => return Ok(ApiResponse::err("找不到对应的订阅记录", 404)),
    };

    let (format, outbounds, user_info, raw_text) = match fetch_and_parse(&sub.url, sub.user_agent.as_deref()).await {
        Ok(res) => res,
        Err(e) => return Ok(ApiResponse::err(e, 400)),
    };

    save_raw_subscription(&id, &raw_text);

    let format_str = match format {
        SubscriptionFormat::SingboxJson => "singbox",
        SubscriptionFormat::ClashYaml => "clash",
        SubscriptionFormat::Base64Uri => "v2ray",
        SubscriptionFormat::Unknown => "unknown",
    }
    .to_string();

    let outbounds = apply_filter_rules(outbounds, sub.filter_rule.as_ref());

    let node_count = match build_and_apply_config(&app_handle, outbounds, sidecar_manager.inner()).await {
        Ok(n) => n,
        Err(e) => return Ok(ApiResponse::err(e, 500)),
    };

    for s in all_subs.iter_mut() {
        if s.id == id {
            s.last_updated = Some(chrono::Utc::now().timestamp_millis());
            s.node_count = Some(node_count);
            s.format = format_str.clone();
            s.is_active = true;
            if user_info.is_some() {
                s.user_info = user_info.clone();
            }
        } else {
            s.is_active = false;
        }
    }
    let _ = save_subscriptions(&all_subs);

    let updated = all_subs
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .unwrap_or(sub);
    Ok(ApiResponse::ok(updated))
}

/// 切换（激活）订阅 — 重新拉取目标订阅 URL 并替换当前运行配置
#[tauri::command]
pub async fn subscription_activate(
    app_handle: AppHandle,
    id: String,
    sidecar_manager: State<'_, Arc<SidecarManager>>,
) -> Result<ApiResponse<Subscription>, AppError> {
    log::info!("[subscription] 激活/切换订阅: {}", id);

    let mut all_subs = load_subscriptions();
    let sub = match all_subs.iter().find(|s| s.id == id) {
        Some(s) => s.clone(),
        None => return Ok(ApiResponse::err("找不到对应的订阅记录", 404)),
    };

    let (format, outbounds, user_info, raw_text) = match fetch_and_parse(&sub.url, sub.user_agent.as_deref()).await {
        Ok(res) => res,
        Err(e) => {
            log::error!("[subscription] 切换订阅拉取失败: {}", e);
            return Ok(ApiResponse::err(e, 400));
        }
    };

    save_raw_subscription(&id, &raw_text);

    let format_str = match format {
        SubscriptionFormat::SingboxJson => "singbox",
        SubscriptionFormat::ClashYaml => "clash",
        SubscriptionFormat::Base64Uri => "v2ray",
        SubscriptionFormat::Unknown => "unknown",
    }
    .to_string();

    let outbounds = apply_filter_rules(outbounds, sub.filter_rule.as_ref());

    let node_count = match build_and_apply_config(&app_handle, outbounds, sidecar_manager.inner()).await {
        Ok(n) => n,
        Err(e) => {
            log::error!("[subscription] 切换订阅应用配置失败: {}", e);
            return Ok(ApiResponse::err(e, 500));
        }
    };

    for s in all_subs.iter_mut() {
        if s.id == id {
            s.is_active = true;
            s.last_updated = Some(chrono::Utc::now().timestamp_millis());
            s.node_count = Some(node_count);
            s.format = format_str.clone();
            if user_info.is_some() {
                s.user_info = user_info.clone();
            }
        } else {
            s.is_active = false;
        }
    }
    let _ = save_subscriptions(&all_subs);

    let updated = all_subs
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .unwrap_or(sub);
    Ok(ApiResponse::ok(updated))
}

/// 直接从纯文本内容或本地文件导入
#[tauri::command]
pub async fn subscription_import_content(
    app_handle: AppHandle,
    name: String,
    content: String,
    source_type: String,
    file_path: Option<String>,
    _auto_group: bool,
    sidecar_manager: State<'_, Arc<SidecarManager>>,
) -> Result<ApiResponse<Subscription>, AppError> {
    log::info!("[subscription] 导入自定义文本/文件: {} ({})", name, source_type);

    let (format, outbounds) = match parse_subscription_content(&content) {
        Ok(res) => res,
        Err(e) => {
            log::error!("[subscription] 解析文本内容失败: {}", e);
            return Ok(ApiResponse::err(e, 400));
        }
    };

    let format_str = match format {
        SubscriptionFormat::SingboxJson => "singbox",
        SubscriptionFormat::ClashYaml => "clash",
        SubscriptionFormat::Base64Uri => "v2ray",
        SubscriptionFormat::Unknown => "unknown",
    }
    .to_string();

    let node_count = match build_and_apply_config(&app_handle, outbounds, sidecar_manager.inner()).await {
        Ok(n) => n,
        Err(e) => {
            log::error!("[subscription] 应用配置失败: {}", e);
            return Ok(ApiResponse::err(e, 500));
        }
    };

    let mut all_subs = load_subscriptions();
    for s in all_subs.iter_mut() {
        s.is_active = false;
    }

    let sub = Subscription {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url: file_path.clone().unwrap_or_else(|| "clipboard://local".to_string()),
        format: format_str,
        source_type,
        local_file_path: file_path,
        user_agent: None,
        auto_update_interval_hours: None,
        last_updated: Some(chrono::Utc::now().timestamp_millis()),
        node_count: Some(node_count),
        is_active: true,
        user_info: None,
        filter_rule: None,
    };

    save_raw_subscription(&sub.id, &content);
    all_subs.push(sub.clone());
    if let Err(e) = save_subscriptions(&all_subs) {
        log::warn!("[subscription] 持久化订阅列表失败: {}", e);
    }

    Ok(ApiResponse::ok(sub))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubscriptionInspectData {
    pub id: String,
    pub name: String,
    pub format: String,
    pub node_count: usize,
    /// 清洗前原始文本
    pub raw_content: String,
    /// 清洗解析后的出站节点列表
    pub parsed_nodes: Vec<crate::core::parser::ParsedOutbound>,
    /// 该订阅生效后的 sing-box 完整运行时配置 JSON
    pub final_config_json: String,
}

/// 检查订阅详情：包含清洗前原始数据、清洗后节点与最终配置预览（支持全平台查看）
#[tauri::command]
pub async fn subscription_inspect(
    app_handle: AppHandle,
    id: String,
) -> Result<ApiResponse<SubscriptionInspectData>, AppError> {
    let all_subs = load_subscriptions();
    let sub = match all_subs.iter().find(|s| s.id == id) {
        Some(s) => s.clone(),
        None => return Ok(ApiResponse::err("找不到对应的订阅记录", 404)),
    };

    let raw_content = load_raw_subscription(&id).unwrap_or_else(|| {
        if sub.source_type == "remote" {
            format!("# 暂无本地原始数据缓存\n# 订阅 URL: {}", sub.url)
        } else {
            "# 本地/剪贴板导入数据".to_string()
        }
    });

    let (_, outbounds) = parse_subscription_content(&raw_content).unwrap_or((SubscriptionFormat::Unknown, Vec::new()));
    let filtered_outbounds = apply_filter_rules(outbounds, sub.filter_rule.as_ref());
    let node_count = filtered_outbounds.len();

    let settings = crate::commands::settings::settings_get_internal(&app_handle);
    let (mixed_port, clash_api_port) = crate::speedtest::get_configured_ports(&app_handle);
    let config_builder = ConfigBuilder::new(filtered_outbounds.clone())
        .with_ports(mixed_port, clash_api_port)
        .with_allow_lan(settings.allow_lan);
    let final_config = config_builder.build().unwrap_or_default();
    let final_config_json = serde_json::to_string_pretty(&final_config).unwrap_or_default();

    Ok(ApiResponse::ok(SubscriptionInspectData {
        id: sub.id,
        name: sub.name,
        format: sub.format,
        node_count,
        raw_content,
        parsed_nodes: filtered_outbounds,
        final_config_json,
    }))
}

/// 修改订阅基础属性（别名、URL、自定义 UA、自动更新周期）
#[tauri::command]
pub async fn subscription_update_meta(
    id: String,
    name: Option<String>,
    url: Option<String>,
    user_agent: Option<String>,
    auto_update_interval_hours: Option<u32>,
    filter_rule: Option<SubscriptionFilterRule>,
) -> ApiResponse<Subscription> {
    let mut all_subs = load_subscriptions();
    let mut updated_sub = None;

    if let Some(s) = all_subs.iter_mut().find(|s| s.id == id) {
        if let Some(n) = name { s.name = n; }
        if let Some(u) = url { s.url = u; }
        s.user_agent = user_agent;
        s.auto_update_interval_hours = auto_update_interval_hours;
        if filter_rule.is_some() {
            s.filter_rule = filter_rule;
        }
        updated_sub = Some(s.clone());
    }

    if let Some(sub) = updated_sub {
        if let Err(e) = save_subscriptions(&all_subs) {
            return ApiResponse::err(format!("保存订阅属性失败: {}", e), 500);
        }
        ApiResponse::ok(sub)
    } else {
        ApiResponse::err("找不到对应订阅", 404)
    }
}




/// 对节点列表应用过滤与重命名清洗规则
pub fn apply_filter_rules(
    mut outbounds: Vec<crate::core::parser::ParsedOutbound>,
    rule: Option<&SubscriptionFilterRule>,
) -> Vec<crate::core::parser::ParsedOutbound> {
    let rule = match rule {
        Some(r) => r,
        None => return outbounds,
    };

    // 1. 排除规则 (例如 官网|重置|流量|客服)
    if let Some(ref exc) = rule.exclude_pattern {
        if !exc.trim().is_empty() {
            if let Ok(re) = regex::Regex::new(exc.trim()) {
                outbounds.retain(|node| !re.is_match(&node.tag));
            }
        }
    }

    // 2. 包含规则 (例如 香港|日本|新加坡|美国)
    if let Some(ref inc) = rule.include_pattern {
        if !inc.trim().is_empty() {
            if let Ok(re) = regex::Regex::new(inc.trim()) {
                outbounds.retain(|node| re.is_match(&node.tag));
            }
        }
    }

    // 3. 正则重命名规则
    if let (Some(ref pat), Some(ref rep)) = (&rule.rename_pattern, &rule.rename_replace) {
        if !pat.trim().is_empty() {
            if let Ok(re) = regex::Regex::new(pat.trim()) {
                for node in outbounds.iter_mut() {
                    node.tag = re.replace_all(&node.tag, rep.as_str()).to_string();
                }
            }
        }
    }

    outbounds
}

/// 启动后台订阅自动静默更新调度器
pub fn start_auto_update_scheduler(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1800)); // 每 30 分钟轮询一次
        loop {
            interval.tick().await;
            let subs = load_subscriptions();
            let now_ms = chrono::Utc::now().timestamp_millis();

            for sub in subs {
                if let Some(hours) = sub.auto_update_interval_hours {
                    if hours > 0 && sub.source_type == "remote" && sub.is_active {
                        let interval_ms = (hours as i64) * 3600 * 1000;
                        let last_up = sub.last_updated.unwrap_or(0);
                        if now_ms - last_up >= interval_ms {
                            log::info!("[auto_updater] 订阅 {} 到达静默更新周期 ({} 小时)，开始后台更新...", sub.name, hours);
                            let app_clone = app_handle.clone();
                            let sub_id = sub.id.clone();
                            if let Some(sidecar_mgr) = app_handle.try_state::<Arc<SidecarManager>>() {
                                let _ = subscription_refresh(app_clone, sub_id, sidecar_mgr).await;
                            }
                        }
                    }
                }
            }
        }
    });
}
