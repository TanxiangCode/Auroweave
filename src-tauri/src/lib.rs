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

/// 获取统一数据根目录：Windows 为 C:\ProgramData\Auroweave，macOS 为 ~/Library/Application Support/Auroweave
/// 服务、GUI 主程序共用此目录，SYSTEM 用户和普通用户均可访问。
pub fn get_data_root() -> std::path::PathBuf {
    #[cfg(target_os = "windows")]
    {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        std::path::PathBuf::from(program_data).join("Auroweave")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        std::path::PathBuf::from(home).join("Library").join("Application Support").join("Auroweave")
    }
    #[cfg(target_os = "linux")]
    {
        std::path::PathBuf::from("/var/lib/Auroweave")
    }
}

/// 获取配置子目录：%ProgramData%\Auroweave\config\
pub fn get_config_dir() -> std::path::PathBuf {
    get_data_root().join("config")
}

/// 获取日志子目录：%ProgramData%\Auroweave\logs\
pub fn get_log_dir() -> std::path::PathBuf {
    get_data_root().join("logs")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 确保日志目录存在
    let log_dir = get_log_dir();
    let _ = std::fs::create_dir_all(&log_dir);

    let sidecar_manager = Arc::new(SidecarManager::new());
    let speedtest_scheduler = Arc::new(SpeedTestScheduler::new());

    tauri::Builder::default()
        .manage(sidecar_manager.clone())
        .manage(speedtest_scheduler.clone())
        // 注册单例插件，确保只运行一个实例
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // 如果尝试启动新实例，将其聚焦（可以触发某些事件）
            let _ = app.get_webview_window("main").map(|w| {
                let _ = w.set_focus();
            });
        }))
        // tauri-plugin-log：前端+后端统一写入同一日志文件
        .plugin(
            tauri_plugin_log::Builder::new()
                // 输出到文件（追加模式）
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Folder {
                        path: log_dir.clone(),
                        file_name: Some("auroweave".to_string()),
                    }
                ))
                // 同时输出到 Webview DevTools 控制台
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Webview
                ))
                // 日志级别：默认 Trace
                .level(log::LevelFilter::Trace)
                // Tauri 内部框架仅记录 Warn 以上，减少噪音
                .level_for("tauri", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .build()
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::proxy::proxy_get_groups,
            commands::proxy::proxy_get_group_nodes,
            commands::proxy::proxy_select_node,
            commands::proxy::proxy_get_mode,
            commands::proxy::proxy_set_mode,
            commands::proxy::proxy_get_singbox_version,
            commands::proxy::sysproxy_set,
            commands::proxy::app_restart_as_admin,
            commands::subscription::subscription_import,
            commands::subscription::subscription_get_all,
            commands::subscription::subscription_delete,
            commands::subscription::subscription_delete_all,
            commands::subscription::subscription_refresh,
            commands::subscription::subscription_activate,
            commands::settings::settings_get_all,
            commands::settings::settings_save,
            commands::settings::settings_inject_terminal_proxy,
            commands::settings::settings_export_diagnostic_log,
            commands::settings::tun_set_enabled,
            commands::settings::service_query_status,
            commands::settings::service_install,
            commands::settings::service_uninstall,
            commands::settings::service_start,
            commands::settings::service_stop,
            commands::settings::service_read_log,
            commands::speedtest::speedtest_run_latency,
            commands::speedtest::speedtest_run_single,
            commands::speedtest::speedtest_run_batch,
            commands::speedtest::speedtest_cancel_batch,
            commands::speedtest::speedtest_get_results,
            commands::routing::routing_get_processes,
            commands::routing::routing_get_app_rules,
            commands::routing::routing_save_app_rule,
            // 日志管理命令
            commands::logging::log_read_app,
            commands::logging::log_read_service,
            commands::logging::log_clear_all,
            commands::settings::core_query_running,
            
            // 流量统计命令
            commands::stats::get_traffic_history,
            commands::stats::get_app_traffic_stats,
        ])
        .setup(move |app| {
            let _window = app.get_webview_window("main")
                .expect("找不到主窗口，请检查 tauri.conf.json 中的窗口配置");

            // 执行旧数据迁移（首次启动时将 %APPDATA% 数据迁移到 %ProgramData%）
            system::startup::migrate_legacy_data(app.handle());

            // 初始化全局 ClashAPI 端口
            let (_, clash_port) = speedtest::get_configured_ports(app.handle());
            core::clash_api::set_clash_api_port(clash_port);

            log::info!("[app] Auroweave 启动 | 数据根目录: {:?}", get_data_root());
            log::info!("[app] Tauri 主窗口已创建");

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = system::startup::apply_core_mode_with_fallback(&app_handle).await {
                    log::error!("[app] 核心自愈与拉起发生错误: {}", e);
                }
            });

            // 启动后台流量监控
            core::traffic_monitor::start_monitor(app.handle().clone());

            // DevTools 暂时关闭，需要调试时取消注释
            // #[cfg(debug_assertions)]
            // {
            //     if let Some(window) = app.get_webview_window("main") {
            //         window.open_devtools();
            //     }
            // }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri 构建失败")
        .run(move |app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                log::info!("[app] 程序正在退出，清理网络代理...");
                // 使用静默模式清理代理，避免退出时弹出 macOS 密码框阻塞退出流程
                let _ = system::sysproxy::set_system_proxy_silent(false, 0);
                let settings = commands::settings::settings_get_internal(app_handle);
                if settings.core.run_mode == "service" {
                    log::info!("[app] 服务模式退出：停止系统服务");
                    let _ = system::service_control::stop_service();
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                } else {
                    log::info!("[app] 直接运行模式退出：停止 sing-box");
                    // Windows: 停止计划任务 TUN；macOS/Linux: 仅停止 sidecar
                    #[cfg(target_os = "windows")]
                    let _ = system::service_control::stop_direct_tun_task();
                    let sidecar_manager = app_handle.state::<std::sync::Arc<SidecarManager>>().inner().clone();
                    // 使用静默停止：macOS TUN 模式下不弹出密码框，
                    // root 进程可能继续运行但会在下次启动时自动清理
                    tauri::async_runtime::block_on(async move {
                        let _ = sidecar_manager.stop_silent().await;
                    });
                }
            }
        });
}
