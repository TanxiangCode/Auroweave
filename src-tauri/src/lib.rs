/// Auroweave — Rust 库入口
/// 作者: TanXiang
pub mod error;
pub mod commands;
pub mod core;
pub mod speedtest;
pub mod system;

use core::sidecar::SidecarManager;
use speedtest::scheduler::SpeedTestScheduler;
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
    let speedtest_scheduler = Arc::new(SpeedTestScheduler::new());

    tauri::Builder::default()
        .manage(sidecar_manager.clone())
        .manage(speedtest_scheduler.clone())
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
            commands::routing::routing_get_processes,
            commands::routing::routing_get_app_rules,
            commands::routing::routing_save_app_rule,
        ])
        .setup(move |app| {
            let _window = app.get_webview_window("main")
                .expect("找不到主窗口，请检查 tauri.conf.json 中的窗口配置");

            // 初始化全局 ClashAPI 端口
            let (_, clash_port) = speedtest::get_configured_ports(app.handle());
            core::clash_api::set_clash_api_port(clash_port);

            tracing::info!("Tauri 窗口已创建");

            let config_dir = app.path().app_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("config"));
            let config_path = config_dir.join("config.json");

            if config_path.exists() {
                let sm = sidecar_manager.clone();
                let path_str = config_path.to_string_lossy().to_string();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = sm.start(&path_str).await {
                        tracing::warn!("启动 sing-box 失败: {}", e);
                    }
                });
            } else {
                tracing::info!("尚未检测到系统配置目录中的 config.json，等待用户导入订阅后拉起");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 启动失败");
}
