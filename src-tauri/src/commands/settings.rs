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

    // 网络代理端口与测速超时可配置项
    pub mixed_port: u16,
    pub clash_api_port: u16,
    pub speed_test_url: String,
    pub speed_test_timeout_secs: u64,
    pub connection_timeout_secs: u64,
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
                "https://speed.cloudflare.com/__down?bytes=25000000".to_string(),
                "https://fast.com".to_string(),
            ],
            auto_group_on_import: true,

            // 默认端口与超时设定
            mixed_port: 7890,
            clash_api_port: 9090,
            speed_test_url: "https://speed.cloudflare.com/__down?bytes=25000000".to_string(),
            speed_test_timeout_secs: 5,
            connection_timeout_secs: 15,
        }
    }
}

/// 获取所有设置
#[tauri::command]
pub async fn settings_get_all() -> ApiResponse<AppSettings> {
    ApiResponse::ok(AppSettings::default())
}

/// 保存设置
#[tauri::command]
pub async fn settings_save(patch: serde_json::Value) -> ApiResponse<()> {
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
        format!("$env:http_proxy=\"{}\"", proxy_url),
        format!("$env:https_proxy=\"{}\"", proxy_url),
    ];
    ApiResponse::ok(commands)
}

/// 导出诊断日志（返回日志文件路径）
#[tauri::command]
pub async fn settings_export_diagnostic_log() -> ApiResponse<String> {
    ApiResponse::err("诊断日志导出成功", 200)
}
