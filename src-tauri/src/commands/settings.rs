/// IPC 命令 — 应用设置
/// 作者: TanXiang
use crate::error::ApiResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub theme: String,
    pub language: String,
    pub proxy_mode: String,
    pub auto_start: bool,
    pub tun_enabled: bool,
    pub topology_enabled: bool,
    pub performance_mode: bool,
    pub command_palette_hotkey: String,
    pub speed_test_urls: Vec<String>,
    pub auto_group_on_import: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "zh-CN".to_string(),
            proxy_mode: "rule".to_string(),
            auto_start: false,
            tun_enabled: false,
            topology_enabled: false,
            performance_mode: false,
            command_palette_hotkey: "CommandOrControl+Space".to_string(),
            speed_test_urls: vec![
                "https://speed.cloudflare.com/__down?bytes=10000000".to_string(),
                "https://fast.com".to_string(),
            ],
            auto_group_on_import: true,
        }
    }
}

/// 获取所有设置
#[tauri::command]
pub async fn settings_get_all() -> ApiResponse<AppSettings> {
    // TODO(模块H): 从本地配置文件加载，合并默认值
    ApiResponse::ok(AppSettings::default())
}

/// 保存设置（前端传入部分字段的 JSON patch）
#[tauri::command]
pub async fn settings_save(patch: serde_json::Value) -> ApiResponse<()> {
    // TODO(模块H): 合并 patch 到当前设置并持久化
    tracing::info!("保存设置: {:?}", patch);
    ApiResponse::ok(())
}

/// 生成终端代理环境变量注入命令（开发者工具箱）
#[tauri::command]
pub async fn settings_inject_terminal_proxy(
    host: String,
    port: u16,
) -> ApiResponse<Vec<String>> {
    let proxy_url = format!("http://{}:{}", host, port);
    let commands = vec![
        format!("export http_proxy={}", proxy_url),
        format!("export https_proxy={}", proxy_url),
        format!("export all_proxy={}", proxy_url),
        // Windows PowerShell 格式
        format!("$env:http_proxy=\"{}\"", proxy_url),
        format!("$env:https_proxy=\"{}\"", proxy_url),
    ];
    ApiResponse::ok(commands)
}

/// 导出诊断日志（返回日志文件路径）
#[tauri::command]
pub async fn settings_export_diagnostic_log() -> ApiResponse<String> {
    // TODO(模块C): 收集 tracing 日志文件路径并返回
    ApiResponse::err("诊断日志导出功能开发中", 501)
}
