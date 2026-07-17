use std::sync::Arc;
use tauri::Manager;
use log::{info, warn, error};

/// 迁移旧版配置数据到统一配置目录
///
/// 迁移流程：
/// 1. 创建统一配置目录（`%ProgramData%\Auroweave\config` 或 Tauri 配置目录）
/// 2. 清理旧版冗余的 `%ProgramData%\Auroweave\config.json`
/// 3. 获取旧版 Tauri 默认配置目录（`app_config_dir`）
/// 4. 若旧目录不存在或与新目录相同，跳过迁移
/// 5. 检查迁移标记文件 `.migrated`，若已迁移过则跳过
/// 6. 复制 config.json / settings.json / config.backup.json 到新目录
/// 7. 写入迁移完成标记，避免重复迁移
pub fn migrate_legacy_data(app_handle: &tauri::AppHandle) {
    // 步骤1: 创建统一配置目录
    let new_config_dir = crate::get_config_dir();
    let _ = std::fs::create_dir_all(&new_config_dir);

    // 步骤2: 清理旧版冗余的 %ProgramData%\Auroweave\config.json
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let redundant_config = std::path::PathBuf::from(program_data).join("Auroweave").join("config.json");
    if redundant_config.exists() {
        let _ = std::fs::remove_file(redundant_config);
    }

    // 步骤3: 获取旧版 Tauri 默认配置目录
    let old_dir = match app_handle.path().app_config_dir() {
        Ok(d) => d,
        Err(_) => return,
    };

    // 步骤4: 若旧目录不存在或与新目录相同，跳过迁移
    if !old_dir.exists() || old_dir == new_config_dir {
        return;
    }

    // 步骤5: 检查迁移标记文件，若已迁移过则跳过
    let migration_flag = new_config_dir.join(".migrated");
    if migration_flag.exists() {
        return;
    }

    info!("[migrate] 检测到旧数据目录: {:?}，开始迁移至统一目录...", old_dir);

    // 步骤6: 逐个复制配置文件（仅在目标不存在时复制，避免覆盖）
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

    // 步骤7: 写入迁移完成标记，避免重复迁移
    let _ = std::fs::write(&migration_flag, "migrated");
    info!("[migrate] 数据迁移完成，统一目录: {:?}", new_config_dir);
}

/// 根据当前设置应用核心运行模式（本地/服务模式 + TUN 开关）
///
/// 这是应用启动和模式切换时的核心编排函数，完整流程如下：
///
/// 1. **前置检查**：确认 config.json 存在，否则跳过拉起
/// 2. **重建配置**：根据最新 settings 重建 config.json
/// 3. **统一清理**：无论之前处于什么状态，先停止系统代理、TUN 计划任务、sing-box 进程
/// 4. **分支决策**：根据 run_mode 分为 service / local 两条路径
///
/// **service 模式路径**：
///   - 检查系统服务状态（未安装/已停止/运行中）
///   - 未安装或启动失败 → 回退为 local 模式（记录 fallback_reason）
///   - 服务就绪后同步配置到服务端
///   - 若 proxy_mode=direct 且 TUN 关闭 → 通知服务停止内核，直接返回
///   - fallback 路径：修改 run_mode=local 并持久化，再走 local 模式拉起逻辑
///
/// **local 模式路径**：
///   - 若 proxy_mode=direct 且 TUN 关闭 → sing-box 保持停止，直接返回
///   - TUN 启用：通过计划任务静默提权拉起 sing-box（需要管理员权限）
///     - 拉起失败 → 回退 tun_enabled=false，改用普通系统代理
///   - TUN 未启用：直接启动 sing-box 子进程 + 设置系统代理
///
/// **竞态修复**：每次修改 settings 时，先重新从磁盘读取最新值，修改后立即持久化，
/// 避免本地变量与磁盘不一致的竞态问题。
pub async fn apply_core_mode_with_fallback(
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    // ---- 步骤1: 前置检查，确认 config.json 存在 ----
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        info!("[app] 尚未检测到 config.json，跳过拉起");
        return Ok(());
    }

    // ---- 步骤2: 根据最新 settings 重建 config.json ----
    // 端口、TUN 等配置可能已变化，需要重建后再拉起
    let _ = crate::commands::settings::rebuild_config_from_settings(app_handle);
    // 每次读取最新的 settings，避免使用过期的本地缓存
    let settings = crate::commands::settings::settings_get_internal(app_handle);
    let sm = app_handle.state::<Arc<crate::core::sidecar::SidecarManager>>().inner().clone();
    let path_str = config_path.to_string_lossy().to_string();

    info!("[app] 检测到 config.json，当前运行模式: {}", settings.core.run_mode);

    // ---- 步骤3: 统一清理旧状态 ----
    // 无论之前处于什么模式，先释放系统代理、停止 TUN 计划任务、停止 sing-box 进程
    // 确保后续拉起在一个干净的环境中进行
    let _ = crate::system::sysproxy::set_system_proxy(false, 0);
    let _ = crate::system::service_control::stop_direct_tun_task();
    let _ = sm.stop().await;

    if settings.core.run_mode == "service" {
        // ============ 服务模式路径 ============
        // ---- 步骤4a: 服务模式分支 ----
        // 检查系统服务状态，决定是直接使用服务还是回退本地模式
        let status = crate::system::service_control::query_service_status().unwrap_or_default();
        info!("[app] 系统服务当前状态: {}", status);

        let mut fallback = false;
        if status == "not_installed" {
            // 情况1: 服务未安装 → 记录原因，回退 local 模式
            warn!("[app] 系统服务未安装，回退为本地运行模式");
            // 原子更新：重新读取最新 settings 再修改，避免竞态
            persist_settings_patch(app_handle, |s| {
                s.core.service.last_fallback_reason = Some("not_installed".to_string());
            });
            fallback = true;
        } else {
            // 情况2: 服务已安装，尝试启动并同步配置
            if status == "stopped" {
                // 服务已停止，先启动服务
                info!("[app] 系统服务已停止，正在启动...");
                if let Err(e) = crate::system::service_control::start_service() {
                    error!("[app] 系统服务启动失败: {}，回退为本地运行模式", e);
                    persist_settings_patch(app_handle, |s| {
                        s.core.service.last_fallback_reason = Some("start_failed".to_string());
                    });
                    fallback = true;
                }
            }
            if !fallback {
                // 服务就绪后等待 500ms 再同步配置，确保 IPC 管道已就绪
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                info!("[app] 向系统服务同步配置文件...");
                if let Err(e) = crate::commands::settings::sync_config_to_service(app_handle).await {
                    error!("[app] 配置同步至服务失败: {}，回退为本地运行模式", e);
                    persist_settings_patch(app_handle, |s| {
                        s.core.service.last_fallback_reason = Some("sync_failed".to_string());
                    });
                    fallback = true;
                }
            }
        }

        // ---- 步骤4a-1: 服务模式下的直连快速路径 ----
        // 若 proxy_mode=direct 且 TUN 关闭，通知服务端停止内核即可，无需拉起
        let current_settings = crate::commands::settings::settings_get_internal(app_handle);
        if current_settings.proxy_mode == "direct" && !current_settings.tun_enabled {
            info!("[app] 服务模式下：系统处于直连且TUN关闭，通知服务停止内核");
            let _ = crate::core::ipc_client::send_ipc_request("SHUTDOWN_CORE", None).await;
            let _ = crate::system::sysproxy::set_system_proxy(false, 0);
            return Ok(());
        }

        if fallback {
            // ---- 步骤4a-2: 服务模式失败，回退 local 模式拉起 ----
            // 原子更新 run_mode=local，保留 fallback_reason（不回滚 tun_enabled）
            persist_settings_patch(app_handle, |s| {
                s.core.run_mode = "local".to_string();
            });
            info!("[app] 已回退为本地运行模式，开始 local 模式拉起流程...");

            // 重新读取最新 settings 进行后续判断
            let fb_settings = crate::commands::settings::settings_get_internal(app_handle);
            let port = fb_settings.mixed_port;

            if fb_settings.tun_enabled {
                // local 模式 + TUN：通过计划任务静默提权
                if let Err(e) = crate::system::service_control::run_direct_tun_task(app_handle) {
                    error!("[app] 回退直接模式时静默拉起 TUN 失败: {}", e);
                    // TUN 拉起失败，回退为普通系统代理模式
                    persist_settings_patch(app_handle, |s| {
                        s.tun_enabled = false;
                    });
                    spawn_local_start(sm.clone(), path_str.clone(), port);
                } else {
                    info!("[app] 回退直接模式 TUN 计划任务触发成功，等待进程启动...");
                    if !wait_for_singbox_running().await {
                        error!("[app] 本地运行模式下静默提权启动 TUN 失败: 进程未运行");
                        persist_settings_patch(app_handle, |s| {
                            s.tun_enabled = false;
                        });
                        spawn_local_start(sm.clone(), path_str.clone(), port);
                    }
                }
            } else {
                // local 模式无 TUN：直接启动子进程
                let _ = sm.start(&path_str).await;
                let _ = crate::system::sysproxy::set_system_proxy(true, port);
            }
            return Ok(());
        }

        // ---- 步骤4a-3: 服务模式成功，设置系统代理 ----
        info!("[app] 配置已成功同步至系统服务");
        let final_settings = crate::commands::settings::settings_get_internal(app_handle);
        if final_settings.tun_enabled {
            // TUN 模式下系统代理不需要开启（流量已被 TUN 接管）
            let _ = crate::system::sysproxy::set_system_proxy(false, 0);
        } else {
            let _ = crate::system::sysproxy::set_system_proxy(true, final_settings.mixed_port);
        }
    } else {
        // ---- 步骤4b: 本地直接运行模式分支 ----
        info!("[app] 直接运行模式 | TUN 已启用: {}", settings.tun_enabled);

        // 直连且 TUN 关闭 → sing-box 保持停止，完全释放网络
        // 这是纯直连模式，不启动任何代理进程
        if settings.proxy_mode == "direct" && !settings.tun_enabled {
            info!("[app] 系统处于直连且TUN关闭，sing-box 保持停止");
            return Ok(());
        }

        let port = settings.mixed_port;

        if settings.tun_enabled {
            // local 模式 + TUN：通过计划任务静默提权拉起 sing-box
            if let Err(e) = crate::system::service_control::run_direct_tun_task(app_handle) {
                error!("[app] 启动时直接模式下静默拉起 TUN 失败: {}", e);
                // TUN 拉起失败，回退为普通系统代理模式
                persist_settings_patch(app_handle, |s| {
                    s.tun_enabled = false;
                });
                spawn_local_start(sm.clone(), path_str.clone(), port);
                return Err(format!("静默提权任务启动失败，已回退为普通系统代理模式: {}", e));
            } else {
                info!("[app] 计划任务 TUN 触发成功，等待进程启动...");
                if wait_for_singbox_running().await {
                    info!("[app] 本地运行模式下通过计划任务静默提权启动 TUN 成功");
                } else {
                    error!("[app] 本地运行模式下静默提权启动 TUN 失败: 进程未运行");
                    persist_settings_patch(app_handle, |s| {
                        s.tun_enabled = false;
                    });
                    spawn_local_start(sm.clone(), path_str.clone(), port);
                    return Err("计划任务启动成功，但内核进程未见运行（可能配置错误或防病毒扫描延迟），已回退为系统代理".to_string());
                }
            }
        } else {
            // local 模式无 TUN：直接启动 sing-box 子进程
            if let Err(e) = sm.start(&path_str).await {
                warn!("[app] 启动 sing-box 失败: {}", e);
                return Err(format!("启动内核进程失败: {}", e));
            } else {
                info!("[app] sing-box 进程已拉起");
                let _ = crate::system::sysproxy::set_system_proxy(true, port);
            }
        }
    }
    Ok(())
}

/// 原子性地更新 settings：读取最新值 → 应用闭包修改 → 持久化保存
///
/// 解决竞态问题：避免使用过期的本地 settings 变量直接修改并保存，
/// 而是每次都从磁盘读取最新值，确保并发修改不会互相覆盖。
fn persist_settings_patch<F>(app_handle: &tauri::AppHandle, patch_fn: F)
where
    F: FnOnce(&mut crate::commands::settings::AppSettings),
{
    let mut settings = crate::commands::settings::settings_get_internal(app_handle);
    patch_fn(&mut settings);
    // 使用 unwrap_or_default 避免 panic，序列化失败时用空对象兜底
    let patch_value = serde_json::to_value(&settings).unwrap_or(serde_json::json!({}));
    if let Err(e) = crate::commands::settings::update_settings_internal(app_handle, patch_value) {
        error!("[app] 持久化 settings 更新失败: {}", e);
    }
}

/// 在后台异步启动 sing-box 子进程并设置系统代理
///
/// 用于 TUN 拉起失败后的回退场景：在后台 spawn 一个任务启动普通代理模式，
/// 避免阻塞当前调用链。
fn spawn_local_start(sm: Arc<crate::core::sidecar::SidecarManager>, path: String, port: u16) {
    tokio::spawn(async move {
        let _ = sm.start(&path).await;
        let _ = crate::system::sysproxy::set_system_proxy(true, port);
    });
}

/// 轮询等待 sing-box 进程启动（最多 5 秒）
///
/// 在通过计划任务静默提权启动 TUN 后，需要等待进程实际运行起来。
/// 每 200ms 检测一次，最多检测 25 次（共 5 秒）。
async fn wait_for_singbox_running() -> bool {
    for _ in 0..25 {
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        if crate::system::service_control::query_singbox_process_running() {
            return true;
        }
    }
    false
}
