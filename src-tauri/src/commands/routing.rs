/// IPC 命令 — App-Matrix 应用分流路由
/// 作者: TanXiang
use crate::error::{ApiResponse, AppError};
use crate::system::process::{get_active_processes, SystemProcess};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;
use tracing::info;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppRuleItem {
    pub process_name: String,
    pub outbound_tag: String,
}

/// 获取当前系统活跃应用进程列表
#[tauri::command]
pub async fn routing_get_processes() -> Result<ApiResponse<Vec<SystemProcess>>, AppError> {
    let list = get_active_processes();
    Ok(ApiResponse::ok(list))
}

/// 获取 app-rules.json 持久化路径
pub fn get_app_rules_path() -> std::path::PathBuf {
    crate::get_config_dir().join("app-rules.json")
}

/// 内部加载应用分流规则函数（供配置生成器与设置重构函数复用）
pub fn load_app_rules_internal() -> HashMap<String, String> {
    let rules_file = get_app_rules_path();
    if !rules_file.exists() {
        return HashMap::new();
    }
    let content = fs::read_to_string(&rules_file).unwrap_or_else(|_| "{}".to_string());
    serde_json::from_str(&content).unwrap_or_default()
}

/// 获取已保存的 App-Matrix 进程规则
#[tauri::command]
pub async fn routing_get_app_rules(_app_handle: AppHandle) -> Result<ApiResponse<HashMap<String, String>>, AppError> {
    Ok(ApiResponse::ok(load_app_rules_internal()))
}

/// 保存单个应用的进程分流规则
#[tauri::command]
pub async fn routing_save_app_rule(
    app_handle: AppHandle,
    process_name: String,
    outbound_tag: String,
) -> Result<ApiResponse<()>, AppError> {
    info!("设置应用 [{}] 绑定出站: {}", process_name, outbound_tag);
    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);
    let rules_file = get_app_rules_path();

    let mut map = load_app_rules_internal();

    if outbound_tag.is_empty() || outbound_tag == "default" {
        map.remove(&process_name);
    } else {
        map.insert(process_name, outbound_tag);
    }

    let json_str = serde_json::to_string_pretty(&map).unwrap_or_default();
    let _ = fs::write(&rules_file, json_str);

    // 重新根据最新 app-rules 重建 config.json
    let _ = crate::commands::settings::rebuild_config_from_settings(&app_handle);

    // 若内核正在运行，触发配置热重载
    let running_res = crate::commands::settings::core_query_running(app_handle.clone()).await;
    if running_res.data.unwrap_or(false) {
        let config_path = config_dir.join("config.json");
        let config_path_str = config_path.to_string_lossy().to_string();
        let client = crate::core::clash_api::ClashApiClient::default();
        let _ = client.reload_config(&config_path_str).await;
        info!("应用分流规则已更新并热重载至 sing-box");
    }

    Ok(ApiResponse::ok(()))
}

/// 自定义分流规则项模型
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomRuleItem {
    pub id: String,
    pub rule_type: String, // "domain" | "domain_suffix" | "domain_keyword" | "domain_regex" | "ip_cidr"
    pub payload: String,   // 匹配内容
    pub outbound_tag: String, // "proxy" | "direct" | "block" | 节点名称
    pub enabled: bool,
    pub description: Option<String>,
}

/// 获取 custom-rules.json 持久化路径
pub fn get_custom_rules_path() -> std::path::PathBuf {
    crate::get_config_dir().join("custom-rules.json")
}

/// 内部加载自定义分流规则函数（供配置生成器复用）
pub fn load_custom_rules_internal() -> Vec<CustomRuleItem> {
    let rules_file = get_custom_rules_path();
    if !rules_file.exists() {
        return Vec::new();
    }
    let content = fs::read_to_string(&rules_file).unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&content).unwrap_or_default()
}

/// 内部保存自定义规则并触发配置重建与热重载
async fn save_custom_rules_internal(app_handle: &AppHandle, rules: &[CustomRuleItem]) -> Result<(), AppError> {
    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);
    let rules_file = get_custom_rules_path();

    let json_str = serde_json::to_string_pretty(rules).unwrap_or_default();
    fs::write(&rules_file, json_str)
        .map_err(|e| AppError::Config(format!("写入自定义规则失败: {}", e)))?;

    // 重新根据最新规则重建 config.json
    let _ = crate::commands::settings::rebuild_config_from_settings(app_handle);

    // 若内核正在运行，触发配置热重载
    let running_res = crate::commands::settings::core_query_running(app_handle.clone()).await;
    if running_res.data.unwrap_or(false) {
        let config_path = config_dir.join("config.json");
        let config_path_str = config_path.to_string_lossy().to_string();
        let client = crate::core::clash_api::ClashApiClient::default();
        let _ = client.reload_config(&config_path_str).await;
        info!("自定义分流规则已更新并热重载至 sing-box");
    }

    Ok(())
}

/// 获取所有自定义分流规则
#[tauri::command]
pub async fn routing_get_custom_rules(_app_handle: AppHandle) -> Result<ApiResponse<Vec<CustomRuleItem>>, AppError> {
    Ok(ApiResponse::ok(load_custom_rules_internal()))
}

/// 批量保存/排序自定义分流规则
#[tauri::command]
pub async fn routing_save_custom_rules(
    app_handle: AppHandle,
    rules: Vec<CustomRuleItem>,
) -> Result<ApiResponse<()>, AppError> {
    save_custom_rules_internal(&app_handle, &rules).await?;
    Ok(ApiResponse::ok(()))
}

/// 添加或更新单条自定义规则
#[tauri::command]
pub async fn routing_add_custom_rule(
    app_handle: AppHandle,
    rule: CustomRuleItem,
) -> Result<ApiResponse<()>, AppError> {
    let mut rules = load_custom_rules_internal();
    if let Some(pos) = rules.iter().position(|r| r.id == rule.id) {
        rules[pos] = rule;
    } else {
        rules.push(rule);
    }
    save_custom_rules_internal(&app_handle, &rules).await?;
    Ok(ApiResponse::ok(()))
}

/// 删除单条自定义规则
#[tauri::command]
pub async fn routing_delete_custom_rule(
    app_handle: AppHandle,
    id: String,
) -> Result<ApiResponse<()>, AppError> {
    let mut rules = load_custom_rules_internal();
    rules.retain(|r| r.id != id);
    save_custom_rules_internal(&app_handle, &rules).await?;
    Ok(ApiResponse::ok(()))
}

