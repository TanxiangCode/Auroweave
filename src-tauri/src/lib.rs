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
            commands::proxy::sysproxy_set,
            commands::proxy::app_restart_as_admin,
            commands::subscription::subscription_import,
            commands::subscription::subscription_get_all,
            commands::subscription::subscription_delete,
            commands::subscription::subscription_refresh,
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
                // 在启动前，将最新的设置配置项同步到 config.json 中
                let _ = commands::settings::rebuild_config_from_settings(app.handle());

                let settings = commands::settings::settings_get_internal(app.handle());
                let app_handle = app.handle().clone();
                let sm = sidecar_manager.clone();
                let path_str = config_path.to_string_lossy().to_string();

                tauri::async_runtime::spawn(async move {
                    if settings.core.run_mode == "service" {
                        // 系统服务模式自愈
                        let status = crate::system::service_control::query_service_status().unwrap_or_default();
                        if status == "not_installed" {
                            tracing::warn!("系统服务未安装，启动时自动回退为直接运行模式");
                            let mut patch_settings = settings.clone();
                            patch_settings.core.run_mode = "direct".to_string();
                            patch_settings.core.service.last_fallback_reason = Some("not_installed".to_string());
                            let _ = commands::settings::update_settings_internal(&app_handle, serde_json::to_value(patch_settings).unwrap());
                            
                            let _ = sm.start(&path_str).await;
                            if settings.tun_enabled {
                                let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                            } else {
                                let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
                            }
                        } else {
                            if status == "stopped" {
                                if let Err(e) = crate::system::service_control::start_service() {
                                    tracing::error!("系统服务启动失败: {}，回退为直接运行模式", e);
                                    let mut patch_settings = settings.clone();
                                    patch_settings.core.run_mode = "direct".to_string();
                                    patch_settings.core.service.last_fallback_reason = Some("start_failed".to_string());
                                    let _ = commands::settings::update_settings_internal(&app_handle, serde_json::to_value(patch_settings).unwrap());
                                    
                                    let _ = sm.start(&path_str).await;
                                    if settings.tun_enabled {
                                        let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                                    } else {
                                        let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
                                    }
                                    return;
                                }
                            }
                            
                            // 同步配置至服务
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            if let Err(e) = commands::settings::sync_config_to_service(&app_handle).await {
                                tracing::error!("配置同步至服务失败: {}，回退为直接运行模式", e);
                                let mut patch_settings = settings.clone();
                                patch_settings.core.run_mode = "direct".to_string();
                                patch_settings.core.service.last_fallback_reason = Some("start_failed".to_string());
                                let _ = commands::settings::update_settings_internal(&app_handle, serde_json::to_value(patch_settings).unwrap());
                                
                                let _ = sm.start(&path_str).await;
                                if settings.tun_enabled {
                                    let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                                } else {
                                    let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
                                }
                            } else {
                                if settings.tun_enabled {
                                    let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                                } else {
                                    let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
                                }
                            }
                        }
                    } else {
                        // 直接运行模式
                        if settings.tun_enabled {
                            // 启动前同步配置到服务目录，并由计划任务静默拉起
                            let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
                            let cache_dir = std::path::PathBuf::from(program_data).join("Auroweave");
                            let _ = std::fs::create_dir_all(&cache_dir);
                            let cache_config_path = cache_dir.join("config.json");
                            let _ = std::fs::copy(&config_path, &cache_config_path);

                            if let Err(e) = crate::system::service_control::run_direct_tun_task() {
                                tracing::error!("启动时直接模式下静默拉起 TUN 失败: {}", e);
                            }
                            let _ = crate::system::sysproxy::set_system_proxy(false, 0);
                        } else {
                            if let Err(e) = sm.start(&path_str).await {
                                tracing::warn!("启动 sing-box 失败: {}", e);
                            } else {
                                let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
                            }
                        }
                    }
                });
            } else {
                tracing::info!("尚未检测到系统配置目录中的 config.json，等待用户导入订阅后拉起");
            }

            // 使用条件编译：只在开发模式下生效
            #[cfg(debug_assertions)]
            {
                // 获取你的主窗口实例（Tauri默认主窗口标签为 "main"）
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools(); // 自动打开内部调试器
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri 构建失败")
        .run(move |app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                let _ = system::sysproxy::set_system_proxy(false, 0);
                let settings = commands::settings::settings_get_internal(app_handle);
                if settings.core.run_mode == "service" {
                    let _ = system::service_control::stop_service();
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                } else {
                    let _ = system::service_control::stop_direct_tun_task();
                    let sidecar_manager = app_handle.state::<std::sync::Arc<SidecarManager>>().inner().clone();
                    tauri::async_runtime::block_on(async move {
                        let _ = sidecar_manager.stop().await;
                    });
                }
            }
        });
}
