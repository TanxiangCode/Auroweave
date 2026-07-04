/// Auroweave — Rust 库入口
/// 作者: TanXiang
pub mod error;
pub mod commands;
pub mod core;

use core::sidecar::SidecarManager;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("auroweave=debug,tauri=warn")
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Auroweave 启动中...");

    let sidecar_manager = Arc::new(SidecarManager::new());

    tauri::Builder::default()
        .manage(sidecar_manager.clone())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
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
        .setup(move |app| {
            let _window = app.get_webview_window("main")
                .expect("找不到主窗口，请检查 tauri.conf.json 中的窗口配置");

            tracing::info!("Tauri 窗口已创建");

            // 如果已有 config/config.json，使用 Tauri 内置 async runtime 异步拉起 sing-box
            let config_path = PathBuf::from("config/config.json");
            if config_path.exists() {
                let sm = sidecar_manager.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = sm.start("config/config.json").await {
                        tracing::warn!("启动 sing-box 失败: {}", e);
                    }
                });
            } else {
                tracing::info!("尚未检测到 config/config.json，等待用户导入订阅后拉起");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 启动失败");
}
