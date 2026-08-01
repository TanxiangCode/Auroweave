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
use tauri::{AppHandle, State};


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub format: String,
    pub last_updated: Option<i64>,
    pub node_count: Option<u32>,
    #[serde(default)]
    pub is_active: bool,
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

/// 内部核心函数：拉取订阅 URL 内容并解析为 outbounds
async fn fetch_and_parse(url: &str) -> Result<(SubscriptionFormat, Vec<crate::core::parser::ParsedOutbound>), AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Network(format!("创建网络客户端失败: {}", e)))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("拉取订阅失败: {}", e)))?;

    let content = resp
        .text()
        .await
        .map_err(|e| AppError::Network(format!("读取订阅响应失败: {}", e)))?;

    parse_subscription_content(&content)
}

/// 内部核心函数：根据 outbounds 生成 config.json 并热重载/拉起 sing-box
async fn build_and_apply_config(
    app_handle: &AppHandle,
    outbounds: Vec<crate::core::parser::ParsedOutbound>,
    sidecar_manager: &SidecarManager,
) -> Result<u32, AppError> {
    if outbounds.is_empty() {
        return Err(AppError::Config("解析出的节点为空，无法生成配置".to_string()));
    }

    let node_count = outbounds.len() as u32;

    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);

    let (mixed_port, clash_api_port) = crate::speedtest::get_configured_ports(app_handle);
    let config_builder = ConfigBuilder::new(outbounds).with_ports(mixed_port, clash_api_port);
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
    let reload_success = clash_client.reload_config(&config_path_str).await.is_ok();

    if !reload_success {
        log::info!("[subscription] ClashAPI 未响应，尝试拉起 sing-box 子进程...");
        if let Err(e) = sidecar_manager.start(&config_path_str).await {
            log::error!("[subscription] 拉起 sing-box 失败 ({})，尝试自动回滚备份...", e);
            if backup_path.exists() {
                let _ = fs::copy(&backup_path, &config_path);
                let _ = sidecar_manager.start(&config_path_str).await;
            }
            return Err(AppError::Sidecar(format!(
                "启动核心失败，已自动回滚备份: {}",
                e
            )));
        }
    } else {
        log::info!("[subscription] sing-box 已成功通过 ClashAPI 热重载配置");
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
///
/// 完整流程:
/// 1. HTTP 拉取订阅内容 (15s 超时)
/// 2. 自动识别格式 (Singbox JSON / Clash YAML / V2ray Base64)
/// 3. ConfigBuilder 生成 config.json (备份旧配置)
/// 4. 尝试 ClashAPI 热重载 → 失败则拉起子进程 → 再失败则回滚备份
/// 5. 根据 proxy_mode 同步系统代理状态
/// 6. 持久化订阅元信息到 subscriptions.json，并标记为活跃
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
        // 返回已存在的订阅的克隆，前端根据 is_active 决定是否需要刷新或直接覆盖
        return Ok(ApiResponse::ok(existing_sub.clone()));
    }

    // 1+2. 拉取并解析
    let (format, outbounds) = match fetch_and_parse(&url).await {
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
        last_updated: Some(chrono::Utc::now().timestamp_millis()),
        node_count: Some(node_count),
        is_active: true,
    };

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

    if !get_subscriptions_path().exists() {
        return ApiResponse::ok(()); // 文件不存在，无需删除
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

    // 拉取并解析
    let (format, outbounds) = match fetch_and_parse(&sub.url).await {
        Ok(res) => res,
        Err(e) => return Ok(ApiResponse::err(e, 400)),
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
        Err(e) => return Ok(ApiResponse::err(e, 500)),
    };

    // 更新持久化记录（保持原 ID 不变）
    for s in all_subs.iter_mut() {
        if s.id == id {
            s.last_updated = Some(chrono::Utc::now().timestamp_millis());
            s.node_count = Some(node_count);
            s.format = format_str.clone();
            s.is_active = true;
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
///
/// 流程与 refresh 一致，但语义上用于在多个已导入订阅间切换。
/// 切换后所有其他订阅自动标记为非活跃。
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

    // 拉取并解析
    let (format, outbounds) = match fetch_and_parse(&sub.url).await {
        Ok(res) => res,
        Err(e) => {
            log::error!("[subscription] 切换订阅拉取失败: {}", e);
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
            log::error!("[subscription] 切换订阅应用配置失败: {}", e);
            return Ok(ApiResponse::err(e, 500));
        }
    };

    // 更新活跃状态
    for s in all_subs.iter_mut() {
        if s.id == id {
            s.is_active = true;
            s.last_updated = Some(chrono::Utc::now().timestamp_millis());
            s.node_count = Some(node_count);
            s.format = format_str.clone();
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
