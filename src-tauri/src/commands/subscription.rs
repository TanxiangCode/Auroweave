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

/// 导入订阅并生成/热重载/拉起 sing-box
///
/// 完整流程:
/// 1. HTTP 拉取订阅内容 (15s 超时)
/// 2. 自动识别格式 (Singbox JSON / Clash YAML / V2ray Base64)
/// 3. ConfigBuilder 生成 config.json (备份旧配置)
/// 4. 尝试 ClashAPI 热重载 → 失败则拉起子进程 → 再失败则回滚备份
/// 5. 根据 proxy_mode 同步系统代理状态
/// 6. 持久化订阅元信息到 subscriptions.json
#[tauri::command]
pub async fn subscription_import(
    app_handle: AppHandle,
    name: String,
    url: String,
    _auto_group: bool,
    sidecar_manager: State<'_, Arc<SidecarManager>>,
) -> Result<ApiResponse<Subscription>, AppError> {
    log::info!("[subscription] 开始导入订阅: {} (URL: {})", name, url);

    // 1. 网络拉取
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(15)).build() {
        Ok(c) => c,
        Err(e) => {
            log::error!("[subscription] 创建网络客户端失败: {}", e);
            return Ok(ApiResponse::err(AppError::Network(e.to_string()), 500));
        }
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            log::error!("[subscription] 拉取订阅失败: {}", e);
            return Ok(ApiResponse::err(AppError::Network(format!("拉取订阅失败: {}", e)), 502));
        }
    };

    let content = match resp.text().await {
        Ok(t) => t,
        Err(e) => {
            log::error!("[subscription] 读取订阅响应体失败: {}", e);
            return Ok(ApiResponse::err(AppError::Network(format!("读取订阅响应失败: {}", e)), 502));
        }
    };

    // 2. 解析格式
    let (format, outbounds) = match parse_subscription_content(&content) {
        Ok(res) => res,
        Err(e) => {
            log::error!("[subscription] 解析订阅失败: {}", e);
            return Ok(ApiResponse::err(e, 400));
        }
    };

    let format_str = match format {
        SubscriptionFormat::SingboxJson => "singbox",
        SubscriptionFormat::ClashYaml => "clash",
        SubscriptionFormat::Base64Uri => "v2ray",
        SubscriptionFormat::Unknown => "unknown",
    }.to_string();

    let node_count = outbounds.len() as u32;
    log::info!("[subscription] 订阅解析成功，格式: {}, 节点数: {}", format_str, node_count);

    // 确定系统配置目录（统一到 ProgramData）
    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);

    // 3. 生成 config.json
    let (mixed_port, clash_api_port) = crate::speedtest::get_configured_ports(&app_handle);
    let config_builder = ConfigBuilder::new(outbounds)
        .with_ports(mixed_port, clash_api_port);
    let config_json = match config_builder.build() {
        Ok(cfg) => cfg,
        Err(e) => {
            log::error!("[subscription] 构建内核配置失败: {}", e);
            return Ok(ApiResponse::err(e, 500));
        }
    };

    let config_path = config_dir.join("config.json");
    let config_path_str = config_path.to_string_lossy().to_string();
    let backup_path = config_dir.join("config.backup.json");

    // 备份当前配置文件到 config.backup.json
    if config_path.exists() {
        let _ = fs::copy(&config_path, &backup_path);
    }

    if let Err(e) = fs::write(&config_path, serde_json::to_string_pretty(&config_json).unwrap_or_default()) {
        log::error!("[subscription] 写入 config.json 失败: {}", e);
        return Ok(ApiResponse::err(AppError::Io(format!("保存 config.json 失败: {}", e)), 500));
    }

    log::info!("[subscription] 成功导入订阅 {}，解析出 {} 个节点，配置已保存至 {:?}", name, node_count, config_path);

    // 4. 拉起/热重载 sing-box 进程 (失败自动安全回滚)
    let clash_client = ClashApiClient::default();
    let reload_success = clash_client.reload_config(&config_path_str).await.is_ok();
    
    if !reload_success {
        log::info!("[subscription] ClashAPI 未响应，尝试拉起 sing-box 子进程...");
        if let Err(e) = sidecar_manager.start(&config_path_str).await {
            log::error!("[subscription] 新配置拉起 sing-box 失败 ({})，尝试自动回滚备份...", e);
            if backup_path.exists() {
                let _ = fs::copy(&backup_path, &config_path);
                let _ = sidecar_manager.start(&config_path_str).await;
            }
            return Ok(ApiResponse::err(AppError::Sidecar(format!("启动核心失败，已自动回滚备份: {}", e)), 500));
        }
    } else {
        log::info!("[subscription] sing-box 已成功通过 ClashAPI 热重载配置");
    }

    // 5. 根据当前的 proxy_mode 同步系统代理状态
    let settings = crate::commands::settings::settings_get_internal(&app_handle);
    if settings.proxy_mode != "direct" {
        let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
    } else {
        let _ = crate::system::sysproxy::set_system_proxy(false, settings.mixed_port);
    }

    // 6. 持久化订阅元数据到 subscriptions.json
    let sub = Subscription {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
        format: format_str,
        last_updated: Some(chrono::Utc::now().timestamp_millis()),
        node_count: Some(node_count),
    };

    let mut all_subs = load_subscriptions();
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

    // 复用导入逻辑重新拉取
    let result = subscription_import(
        app_handle,
        sub.name.clone(),
        sub.url.clone(),
        true,
        sidecar_manager,
    ).await;

    match result {
        Ok(resp) if resp.success => {
            if let Some(new_sub) = resp.data {
                let new_format = new_sub.format.clone();
                let new_last_updated = new_sub.last_updated;
                let new_node_count = new_sub.node_count;
                // 更新持久化记录（保持原 ID 不变）
                for s in all_subs.iter_mut() {
                    if s.id == id {
                        s.last_updated = new_last_updated;
                        s.node_count = new_node_count;
                        s.format = new_format.clone();
                    }
                }
                let _ = save_subscriptions(&all_subs);
                let mut updated = new_sub;
                updated.id = id;
                Ok(ApiResponse::ok(updated))
            } else {
                Ok(ApiResponse::err("刷新成功但返回数据异常", 500))
            }
        }
        Ok(resp) => Ok(ApiResponse::err(resp.error.unwrap_or_else(|| "刷新订阅失败".to_string()), resp.code.unwrap_or(500))),
        Err(e) => Ok(ApiResponse::err(e.to_string(), 500)),
    }
}
