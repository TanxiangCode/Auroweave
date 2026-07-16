use std::sync::Arc;
use tauri::Manager;
use log::{info, warn, error};

pub fn migrate_legacy_data(app_handle: &tauri::AppHandle) {
    let new_config_dir = crate::get_config_dir();
    let _ = std::fs::create_dir_all(&new_config_dir);

    // 清理旧的冗余的 %ProgramData%\Auroweave\config.json
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let redundant_config = std::path::PathBuf::from(program_data).join("Auroweave").join("config.json");
    if redundant_config.exists() {
        let _ = std::fs::remove_file(redundant_config);
    }

    // 尝试获取旧的 Tauri 默认配置目录
    let old_dir = match app_handle.path().app_config_dir() {
        Ok(d) => d,
        Err(_) => return,
    };

    // 如果旧目录不存在或与新目录相同，跳过迁移
    if !old_dir.exists() || old_dir == new_config_dir {
        return;
    }

    // 迁移标记文件：如果已迁移过则跳过
    let migration_flag = new_config_dir.join(".migrated");
    if migration_flag.exists() {
        return;
    }

    info!("[migrate] 检测到旧数据目录: {:?}，开始迁移至统一目录...", old_dir);

    let files = vec!["config.json", "settings.json", "config.backup.json"];
    for file in files {
        let old_file = old_dir.join(file);
        let new_file = new_config_dir.join(file);
        if old_file.exists() && !new_file.exists() {
            match std::fs::copy(&old_file, &new_file) {
                Ok(_) => info!("[migrate] {} 迁移成功", file),
                Err(e) => warn!("[migrate] {} 迁移失败: {}", file, e),
            }
        }
    }

    // 写入迁移完成标记
    let _ = std::fs::write(&migration_flag, "migrated");
    info!("[migrate] 数据迁移完成，统一目录: {:?}", new_config_dir);
}

pub async fn apply_core_mode_with_fallback(
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        info!("[app] 尚未检测到 config.json，跳过拉起");
        return Ok(());
    }

    let _ = crate::commands::settings::rebuild_config_from_settings(app_handle);
    let mut settings = crate::commands::settings::settings_get_internal(app_handle);
    let sm = app_handle.state::<Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
    let path_str = config_path.to_string_lossy().to_string();

    info!("[app] 检测到 config.json，当前运行模式: {}", settings.core.run_mode);

    // 首先统一停止现有的进程和代理
    let _ = crate::system::sysproxy::set_system_proxy(false, 0);
    let _ = crate::system::service_control::stop_direct_tun_task();
    let _ = sm.stop().await;

    if settings.core.run_mode == "service" {
        let status = crate::system::service_control::query_service_status().unwrap_or_default();
        info!("[app] 系统服务当前状态: {}", status);

        let mut fallback = false;
        if status == "not_installed" {
            warn!("[app] 系统服务未安装，回退为本地运行模式");
            settings.core.service.last_fallback_reason = Some("not_installed".to_string());
            fallback = true;
        } else {
            if status == "stopped" {
                info!("[app] 系统服务已停止，正在启动...");
                if let Err(e) = crate::system::service_control::start_service() {
                    error!("[app] 系统服务启动失败: {}，回退为本地运行模式", e);
                    settings.core.service.last_fallback_reason = Some("start_failed".to_string());
                    fallback = true;
                }
            }
            if !fallback {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                info!("[app] 向系统服务同步配置文件...");
                if let Err(e) = crate::commands::settings::sync_config_to_service(app_handle).await {
                    error!("[app] 配置同步至服务失败: {}，回退为本地运行模式", e);
                    settings.core.service.last_fallback_reason = Some("sync_failed".to_string());
                    fallback = true;
                }
            }
        }

        if settings.proxy_mode == "direct" && !settings.tun_enabled {
            info!("[app] 服务模式下：系统处于直连且TUN关闭，通知服务停止内核");
            let _ = crate::core::ipc_client::send_ipc_request("SHUTDOWN_CORE", None).await;
            let _ = crate::system::sysproxy::set_system_proxy(false, 0);
            return Ok(());
        }

        if fallback {
            settings.core.run_mode = "local".to_string();
            // 不回滚 tun_enabled 状态，因为 local 模式下可能也想用 TUN
            let _ = crate::commands::settings::update_settings_internal(app_handle, serde_json::to_value(&settings).unwrap());
            
            // local 回退拉起
            if settings.tun_enabled {
                if let Err(e) = crate::system::service_control::run_direct_tun_task(app_handle) {
                    error!("[app] 回退直接模式时静默拉起 TUN 失败: {}", e);
                    // 彻底失败，再次回退 tun_enabled = false
                    settings.tun_enabled = false;
                    let _ = crate::commands::settings::update_settings_internal(app_handle, serde_json::to_value(&settings).unwrap());
                    let sm_clone = sm.clone();
                    let path_clone = path_str.clone();
                    let port = settings.mixed_port;
                    tokio::spawn(async move {
                        let _ = sm_clone.start(&path_clone).await;
                        let _ = crate::system::sysproxy::set_system_proxy(true, port);
                    });
                } else {
                    info!("[app] 回退直接模式 TUN 计划任务触发成功，等待进程启动...");
                    let mut is_running = false;
                    for _ in 0..25 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                        if crate::system::service_control::query_singbox_process_running() {
                            is_running = true;
                            break;
                        }
                    }
                    if !is_running {
                        error!("[app] 本地运行模式下静默提权启动 TUN 失败: 进程未运行");
                        settings.tun_enabled = false;
                        let _ = crate::commands::settings::update_settings_internal(app_handle, serde_json::to_value(&settings).unwrap());
                        let sm_clone = sm.clone();
                        let path_clone = path_str.clone();
                        let port = settings.mixed_port;
                        tokio::spawn(async move {
                            let _ = sm_clone.start(&path_clone).await;
                            let _ = crate::system::sysproxy::set_system_proxy(true, port);
                        });
                    }
                }
            } else {
                let _ = sm.start(&path_str).await;
                let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
            }
            return Ok(());
        }

        // 服务模式成功拉起
        info!("[app] 配置已成功同步至系统服务");
        if settings.tun_enabled {
            let _ = crate::system::sysproxy::set_system_proxy(false, 0);
        } else {
            let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
        }
    } else {
        info!("[app] 直接运行模式 | TUN 已启用: {}", settings.tun_enabled);
        
        if settings.proxy_mode == "direct" && !settings.tun_enabled {
            info!("[app] 系统处于直连且TUN关闭，sing-box 保持停止");
            return Ok(());
        }

        if settings.tun_enabled {
            if let Err(e) = crate::system::service_control::run_direct_tun_task(app_handle) {
                error!("[app] 启动时直接模式下静默拉起 TUN 失败: {}", e);
                // 直接模式 TUN 拉起失败，回退为非 TUN 模式
                settings.tun_enabled = false;
                let _ = crate::commands::settings::update_settings_internal(app_handle, serde_json::to_value(&settings).unwrap());
                let sm_clone = sm.clone();
                let path_clone = path_str.clone();
                let port = settings.mixed_port;
                tokio::spawn(async move {
                    let _ = sm_clone.start(&path_clone).await;
                    let _ = crate::system::sysproxy::set_system_proxy(true, port);
                });
                return Err(format!("静默提权任务启动失败，已回退为普通系统代理模式: {}", e));
            } else {
                info!("[app] 计划任务 TUN 触发成功，等待进程启动...");
                let mut is_running = false;
                for _ in 0..25 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    if crate::system::service_control::query_singbox_process_running() {
                        is_running = true;
                        break;
                    }
                }
                
                if is_running {
                    info!("[app] 本地运行模式下通过计划任务静默提权启动 TUN 成功");
                } else {
                    error!("[app] 本地运行模式下静默提权启动 TUN 失败: 进程未运行");
                    settings.tun_enabled = false;
                    let _ = crate::commands::settings::update_settings_internal(app_handle, serde_json::to_value(&settings).unwrap());
                    let sm_clone = sm.clone();
                    let path_clone = path_str.clone();
                    let port = settings.mixed_port;
                    tokio::spawn(async move {
                        let _ = sm_clone.start(&path_clone).await;
                        let _ = crate::system::sysproxy::set_system_proxy(true, port);
                    });
                    return Err("计划任务启动成功，但内核进程未见运行（可能配置错误或防病毒扫描延迟），已回退为系统代理".to_string());
                }
            }
        } else {
            if let Err(e) = sm.start(&path_str).await {
                warn!("[app] 启动 sing-box 失败: {}", e);
                return Err(format!("启动内核进程失败: {}", e));
            } else {
                info!("[app] sing-box 进程已拉起");
                let _ = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port);
            }
        }
    }
    Ok(())
}
