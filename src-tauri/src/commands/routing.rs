/// IPC 命令 — App-Matrix 应用分流路由
/// 作者: TanXiang
use crate::error::{ApiResponse, AppError};
use crate::system::process::{get_active_processes, SystemProcess};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;
use tauri::Manager;
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

/// 获取已保存的 App-Matrix 进程规则
#[tauri::command]
pub async fn routing_get_app_rules(app_handle: AppHandle) -> Result<ApiResponse<HashMap<String, String>>, AppError> {
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("config"));
    let rules_file = config_dir.join("app-rules.json");

    if !rules_file.exists() {
        return Ok(ApiResponse::ok(HashMap::new()));
    }

    let content = fs::read_to_string(&rules_file).unwrap_or_else(|_| "{}".to_string());
    let map: HashMap<String, String> = serde_json::from_str(&content).unwrap_or_default();

    Ok(ApiResponse::ok(map))
}

/// 保存单个应用的进程分流规则
#[tauri::command]
pub async fn routing_save_app_rule(
    app_handle: AppHandle,
    process_name: String,
    outbound_tag: String,
) -> Result<ApiResponse<()>, AppError> {
    info!("设置应用 [{}] 绑定出站: {}", process_name, outbound_tag);
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("config"));
    let _ = fs::create_dir_all(&config_dir);
    let rules_file = config_dir.join("app-rules.json");

    let mut map: HashMap<String, String> = if rules_file.exists() {
        let content = fs::read_to_string(&rules_file).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    if outbound_tag.is_empty() || outbound_tag == "default" {
        map.remove(&process_name);
    } else {
        map.insert(process_name, outbound_tag);
    }

    let json_str = serde_json::to_string_pretty(&map).unwrap_or_default();
    let _ = fs::write(&rules_file, json_str);

    Ok(ApiResponse::ok(()))
}
