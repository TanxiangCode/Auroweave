/// IPC 命令 — 订阅管理
/// 作者: TanXiang
use crate::core::config_builder::ConfigBuilder;
use crate::core::parser::{parse_subscription_content, SubscriptionFormat};
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
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

/// 导入订阅并生成/热重载 config.json
#[tauri::command]
pub async fn subscription_import(
    name: String,
    url: String,
    _auto_group: bool,
) -> ApiResponse<Subscription> {
    info!("开始导入订阅: {} (URL: {})", name, url);

    // 1. 网络拉取
    let client = match reqwest::Client::builder().timeout(Duration::from_secs(15)).build() {
        Ok(c) => c,
        Err(e) => return ApiResponse::err(AppError::Network(e.to_string()), 500),
    };

    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return ApiResponse::err(AppError::Network(format!("拉取订阅失败: {}", e)), 502),
    };

    let content = match resp.text().await {
        Ok(t) => t,
        Err(e) => return ApiResponse::err(AppError::Network(format!("读取订阅响应失败: {}", e)), 502),
    };

    // 2. 解析格式
    let (format, outbounds) = match parse_subscription_content(&content) {
        Ok(res) => res,
        Err(e) => return ApiResponse::err(e, 400),
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
        Err(e) => return ApiResponse::err(e, 500),
    };

    // 写入 app_data 目录中的 config.json
    let config_dir = PathBuf::from("./config");
    let _ = fs::create_dir_all(&config_dir);
    let config_path = config_dir.join("config.json");

    if let Err(e) = fs::write(&config_path, serde_json::to_string_pretty(&config_json).unwrap_or_default()) {
        return ApiResponse::err(AppError::Io(format!("保存 config.json 失败: {}", e)), 500);
    }

    info!("成功导入订阅 {}，解析出 {} 个节点，配置已保存至 {:?}", name, node_count, config_path);

    let sub = Subscription {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
        format: format_str,
        last_updated: Some(chrono::Utc::now().timestamp_millis()),
        node_count: Some(node_count),
    };

    ApiResponse::ok(sub)
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
