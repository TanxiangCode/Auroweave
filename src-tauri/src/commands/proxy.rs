/// IPC 命令 — 代理节点与分组
/// 作者: TanXiang
///
/// 规则：
/// - 此层只做参数校验与调用编排，具体逻辑下沉到 core/ 领域模块
/// - 所有命令返回 ApiResponse<T>
/// - 命名格式：proxy_动作
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};

// ---- 数据结构（与前端 types/index.ts 字段对齐）----

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

// ---- IPC 命令 ----

/// 获取所有代理分组
#[tauri::command]
pub async fn proxy_get_groups() -> ApiResponse<Vec<ProxyGroup>> {
    // TODO(模块B): 通过 ClashAPI HTTP 获取分组列表
    tracing::debug!("proxy_get_groups 调用");
    ApiResponse::ok(vec![])
}

/// 获取分组内所有节点
#[tauri::command]
pub async fn proxy_get_group_nodes(group_tag: String) -> ApiResponse<Vec<ProxyNode>> {
    // TODO(模块B): 从 ClashAPI 获取 /proxies/{name} 数据
    tracing::debug!("proxy_get_group_nodes: {}", group_tag);
    ApiResponse::ok(vec![])
}

/// 切换分组当前节点
#[tauri::command]
pub async fn proxy_select_node(group_tag: String, node_tag: String) -> ApiResponse<()> {
    // TODO(模块B): PUT /proxies/{group_tag} { "name": node_tag }
    tracing::debug!("proxy_select_node: {} -> {}", group_tag, node_tag);
    ApiResponse::ok(())
}

/// 获取当前代理模式
#[tauri::command]
pub async fn proxy_get_mode() -> ApiResponse<String> {
    // TODO(模块B): GET /configs 解析 mode 字段
    ApiResponse::ok("rule".to_string())
}

/// 切换代理模式
#[tauri::command]
pub async fn proxy_set_mode(mode: String) -> ApiResponse<()> {
    if !["global", "rule", "direct"].contains(&mode.as_str()) {
        return ApiResponse::err(
            AppError::Validation(format!("无效的代理模式: {}", mode)),
            400,
        );
    }
    // TODO(模块B): PATCH /configs { "mode": mode }
    tracing::info!("切换代理模式: {}", mode);
    ApiResponse::ok(())
}
