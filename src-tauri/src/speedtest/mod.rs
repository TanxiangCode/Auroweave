/// Auroweave 智能测速模块
/// 作者: TanXiang
pub mod throughput;
pub mod scheduler;

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputResult {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub tested_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgress {
    pub current_index: usize,
    pub total: usize,
    pub current_node: String,
    pub result: Option<ThroughputResult>,
}

/// 从 settings.json 中安全地读取本地代理端口与 ClashAPI 端口，不存在则返回默认 (8890, 9090)
///
/// 注意：必须从 `crate::get_config_dir()`（统一数据根目录）读取，
/// 与 `commands::settings::settings_get_internal` 保持同一路径，
/// 严禁使用 Tauri 的 `app_config_dir()`（其按 bundle identifier 解析，与实际写入路径不一致）。
pub fn get_configured_ports(_app_handle: &tauri::AppHandle) -> (u16, u16) {
    let path = crate::get_config_dir().join("settings.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let m = val.get("mixed_port").and_then(|p| p.as_u64()).unwrap_or(8890).min(65535) as u16;
                let c = val.get("clash_api_port").and_then(|p| p.as_u64()).unwrap_or(9090).min(65535) as u16;
                return (m, c);
            }
        }
    }
    (8890, 9090)
}

/// 从 settings.json 中安全地读取本地代理端口，不存在则返回默认端口 8890
pub fn get_mixed_port(app_handle: &tauri::AppHandle) -> u16 {
    get_configured_ports(app_handle).0
}
