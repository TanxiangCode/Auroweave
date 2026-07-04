/// IPC 命令 — 订阅管理
/// 作者: TanXiang
use crate::error::ApiResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub format: String,
    pub last_updated: Option<i64>,
    pub node_count: Option<u32>,
}

/// 导入订阅
#[tauri::command]
pub async fn subscription_import(
    name: String,
    url: String,
    auto_group: bool,
) -> ApiResponse<Subscription> {
    // TODO(模块B): 拉取订阅内容 → 解析格式 → 生成 config.json
    tracing::info!("导入订阅: {} (auto_group={})", name, auto_group);
    let sub = Subscription {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
        format: "unknown".to_string(),
        last_updated: Some(chrono::Utc::now().timestamp_millis()),
        node_count: None,
    };
    ApiResponse::ok(sub)
}

/// 获取所有已保存订阅
#[tauri::command]
pub async fn subscription_get_all() -> ApiResponse<Vec<Subscription>> {
    // TODO(模块B): 从本地持久化存储读取
    ApiResponse::ok(vec![])
}

/// 删除订阅
#[tauri::command]
pub async fn subscription_delete(id: String) -> ApiResponse<()> {
    // TODO(模块B): 从存储中删除，重新生成 config.json
    tracing::info!("删除订阅: {}", id);
    ApiResponse::ok(())
}

/// 刷新订阅
#[tauri::command]
pub async fn subscription_refresh(id: String) -> ApiResponse<Subscription> {
    // TODO(模块B): 重新拉取对应 URL 并更新本地数据
    tracing::info!("刷新订阅: {}", id);
    ApiResponse::err(
        "订阅刷新功能尚未实现，模块B开发中",
        501,
    )
}
