/// IPC 命令 — 订阅管理
/// 作者: TanXiang
use crate::core::clash_api::ClashApiClient;
use crate::core::config_builder::ConfigBuilder;
use crate::core::parser::{parse_subscription_content, SubscriptionFormat};
use crate::core::sidecar::SidecarManager;
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::State;
use tracing::info;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub format: String,
    pub last_updated: Option<i64>,
    pub node_count: Option<u32>,
}

/// 导入订阅并生成/热重载/拉起 sing-box
#[tauri::command]
pub async fn subscription_import(
    name: String,
    url: String,
    _auto_group: bool,
    sidecar_manager: State<'_, Arc<SidecarManager>>,
) -> Result<ApiResponse<Subscription>, AppError> {
    info!("开始导入订阅: {} (URL: {})", name, url);

    // 1. 网络拉取
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(15)).build() {
        Ok(c) => c,
        Err(e) => return Ok(ApiResponse::err(AppError::Network(e.to_string()), 500)),
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return Ok(ApiResponse::err(AppError::Network(format!("拉取订阅失败: {}", e)), 502)),
    };

    let content = match resp.text().await {
        Ok(t) => t,
        Err(e) => return Ok(ApiResponse::err(AppError::Network(format!("读取订阅响应失败: {}", e)), 502)),
    };

    // 2. 解析格式
    let (format, outbounds) = match parse_subscription_content(&content) {
        Ok(res) => res,
        Err(e) => return Ok(ApiResponse::err(e, 400)),
    };

    let format_str = match format {
        SubscriptionFormat::SingboxJson => "singbox",
        SubscriptionFormat::ClashYaml => "clash",
        SubscriptionFormat::Base64Uri => "v2ray",
        SubscriptionFormat::Unknown => "unknown",
    }.to_string();

    let node_count = outbounds.len() as u32;

    // 3. 生成 config.json
    let config_builder = ConfigBuilder::new(outbounds);
    let config_json = match config_builder.build() {
        Ok(cfg) => cfg,
        Err(e) => return Ok(ApiResponse::err(e, 500)),
    };

    // 保存 config.json 到项目根目录 config/ 文件夹 (位于 src-tauri 外部，避免触发 Cargo watcher 重新构建)
    let config_path = if PathBuf::from("Cargo.toml").exists() && PathBuf::from("../package.json").exists() {
        PathBuf::from("../config/config.json")
    } else {
        PathBuf::from("config/config.json")
    };

    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let config_path_str = config_path.to_string_lossy().to_string();

    if let Err(e) = fs::write(&config_path, serde_json::to_string_pretty(&config_json).unwrap_or_default()) {
        return Ok(ApiResponse::err(AppError::Io(format!("保存 config.json 失败: {}", e)), 500));
    }

    info!("成功导入订阅 {}，解析出 {} 个节点，配置已保存至 {:?}", name, node_count, config_path);

    // 4. 拉起/热重载 sing-box 进程
    let clash_client = ClashApiClient::default();
    if let Err(_) = clash_client.reload_config(&config_path_str).await {
        info!("ClashAPI 未响应，尝试拉起 sing-box 子进程...");
        if let Err(e) = sidecar_manager.start(&config_path_str).await {
            info!("sing-box 启动提示: {}", e);
        }
    } else {
        info!("sing-box 已成功热重载配置");
    }

    let sub = Subscription {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
        format: format_str,
        last_updated: Some(chrono::Utc::now().timestamp_millis()),
        node_count: Some(node_count),
    };

    Ok(ApiResponse::ok(sub))
}

/// 获取所有已保存订阅
#[tauri::command]
pub async fn subscription_get_all() -> ApiResponse<Vec<Subscription>> {
    ApiResponse::ok(vec![])
}

/// 删除订阅
#[tauri::command]
pub async fn subscription_delete(id: String) -> ApiResponse<()> {
    info!("删除订阅: {}", id);
    ApiResponse::ok(())
}

/// 刷新订阅
#[tauri::command]
pub async fn subscription_refresh(id: String) -> ApiResponse<Subscription> {
    info!("刷新订阅: {}", id);
    ApiResponse::err("订阅刷新功能准备就绪", 501)
}
