/// IPC 命令 — 订阅管理
/// 作者: TanXiang
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
    app_handle: AppHandle,
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

    // 确定系统配置目录
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("config"));
    let _ = fs::create_dir_all(&config_dir);
    let log_path_str = config_dir.join("box.log").to_string_lossy().to_string();

    // 3. 生成 config.json
    let (mixed_port, clash_api_port) = crate::speedtest::get_configured_ports(&app_handle);
    let config_builder = ConfigBuilder::new(outbounds)
        .with_ports(mixed_port, clash_api_port)
        .with_log_path(log_path_str);
    let config_json = match config_builder.build() {
        Ok(cfg) => cfg,
        Err(e) => return Ok(ApiResponse::err(e, 500)),
    };

    let config_path = config_dir.join("config.json");
    let config_path_str = config_path.to_string_lossy().to_string();
    let backup_path = config_dir.join("config.backup.json");

    // 备份当前配置文件到 config.backup.json
    if config_path.exists() {
        let _ = fs::copy(&config_path, &backup_path);
    }

    if let Err(e) = fs::write(&config_path, serde_json::to_string_pretty(&config_json).unwrap_or_default()) {
        return Ok(ApiResponse::err(AppError::Io(format!("保存 config.json 失败: {}", e)), 500));
    }

    info!("成功导入订阅 {}，解析出 {} 个节点，配置已保存至 {:?}", name, node_count, config_path);

    // 4. 拉起/热重载 sing-box 进程 (失败自动安全回滚)
    let clash_client = ClashApiClient::default();
    let reload_success = clash_client.reload_config(&config_path_str).await.is_ok();
    
    if !reload_success {
        info!("ClashAPI 未响应，尝试拉起 sing-box 子进程...");
        if let Err(e) = sidecar_manager.start(&config_path_str).await {
            info!("新配置拉起 sing-box 失败 ({})，尝试自动回滚备份...", e);
            if backup_path.exists() {
                let _ = fs::copy(&backup_path, &config_path);
                let _ = sidecar_manager.start(&config_path_str).await;
            }
            return Ok(ApiResponse::err(AppError::Sidecar(format!("启动核心失败，已自动回滚备份: {}", e)), 500));
        }
    } else {
        info!("sing-box 已成功热重载配置");
    }

    // 5. 根据当前的 proxy_mode 同步系统代理状态
    let settings = crate::commands::settings::settings_get_internal(&app_handle);
    if settings.proxy_mode != "direct" {
        let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
    } else {
        let _ = crate::system::sysproxy::set_system_proxy(false, settings.mixed_port);
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
