/// Auroweave — Rust 库入口
/// 作者: TanXiang
///
/// 职责：
/// - 初始化日志系统（tracing）
/// - 构建 Tauri App，注册所有插件与 IPC 命令
/// - 配置系统托盘
pub mod error;
pub mod commands;
pub mod core;

use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志（生产构建可通过 RUST_LOG 环境变量控制级别）
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("auroweave=debug,tauri=warn")
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Auroweave 启动中...");

    tauri::Builder::default()
        // 注册插件
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // 注册所有 IPC 命令
        .invoke_handler(tauri::generate_handler![
            commands::proxy::proxy_get_groups,
            commands::proxy::proxy_get_group_nodes,
            commands::proxy::proxy_select_node,
            commands::proxy::proxy_get_mode,
            commands::proxy::proxy_set_mode,
            commands::subscription::subscription_import,
            commands::subscription::subscription_get_all,
            commands::subscription::subscription_delete,
            commands::subscription::subscription_refresh,
            commands::settings::settings_get_all,
            commands::settings::settings_save,
            commands::settings::settings_inject_terminal_proxy,
            commands::settings::settings_export_diagnostic_log,
            commands::speedtest::speedtest_run_latency,
            commands::speedtest::speedtest_run_single,
            commands::speedtest::speedtest_run_batch,
            commands::speedtest::speedtest_cancel_batch,
            commands::speedtest::speedtest_get_results,
        ])
        .setup(|app| {
            // 获取主窗口并初始化 sing-box 核心
            let _window = app.get_webview_window("main")
                .expect("找不到主窗口，请检查 tauri.conf.json 中的窗口配置");

            tracing::info!("Tauri 窗口已创建，准备启动 sing-box 核心...");
            // TODO(模块B): 启动 sidecar，core::sidecar::start()

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 启动失败");
}
