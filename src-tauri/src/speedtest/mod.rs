/// Auroweave 智能测速模块
/// 作者: TanXiang
pub mod throughput;
pub mod scheduler;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use std::fs;
use std::path::PathBuf;

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
pub fn get_configured_ports(app_handle: &AppHandle) -> (u16, u16) {
    let config_dir = app_handle.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("config"));
    let path = config_dir.join("settings.json");
    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let m = val.get("mixed_port").and_then(|p| p.as_u64()).unwrap_or(8890) as u16;
                let c = val.get("clash_api_port").and_then(|p| p.as_u64()).unwrap_or(9090) as u16;
                return (m, c);
            }
        }
    }
    (8890, 9090)
}

/// 从 settings.json 中安全地读取本地代理端口，不存在则返回默认端口 8890
pub fn get_mixed_port(app_handle: &AppHandle) -> u16 {
    get_configured_ports(app_handle).0
}
