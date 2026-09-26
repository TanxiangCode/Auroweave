/// 系统代理守护 (System Proxy Guard)
/// 作者: TanXiang
///
/// 场景：其他应用修改系统代理设置（或系统重置网络服务）后，浏览器断网而
/// 内核实际还在运行——用户无感知原因。守护循环周期校验，发现漂移自动恢复。
///
/// 冲突仲裁（上轮搁置的核心顾虑的解法）：
/// - 期望状态（desired）由应用内的写入口统一维护：set_desired(true) 后守护生效，
///   用户主动关代理（托盘翻转/sysproxy_set(false)/退出清理）置 desired=false，
///   守护立即停手，不会跟用户的主动操作打架
/// - 恢复动作仅两层校验通过才执行：desired=true 且 内核正在运行（ClashAPI 可达）
/// - 反向同样收敛：系统代理开着但（用户已期望关闭 或 内核已停）一律静默关闭，
///   这是"内核没了 + 残留 127.0.0.1 代理 = 整机断网"的自愈兜底
/// - 恢复失败（如 networksetup 需提权被用户取消）不重试轰炸：降频为下轮再查
use std::sync::atomic::{AtomicBool, Ordering};

/// 用户期望的系统代理状态（应用内所有写入口同步维护）
static DESIRED_ENABLED: AtomicBool = AtomicBool::new(false);

/// 守护循环是否已启动（防重复 spawn）
static GUARD_STARTED: AtomicBool = AtomicBool::new(false);

/// 残留清理告警去重标记（30s 一拍，避免清理持续失败时刷屏）
static RESIDUAL_WARNED: AtomicBool = AtomicBool::new(false);

/// 设置期望状态：应用内开启/关闭系统代理的统一入口
/// （sysproxy 命令、托盘翻转、模式切换、退出清理都应调用）
pub fn set_desired(enabled: bool) {
    DESIRED_ENABLED.store(enabled, Ordering::Release);
}

/// 当前期望状态
pub fn is_desired_enabled() -> bool {
    DESIRED_ENABLED.load(Ordering::Acquire)
}

/// 启动守护循环（幂等；由 lib.rs setup 调用一次）
///
/// 每 30 秒一拍，两个方向收敛系统代理状态：
/// - **漂移恢复**：期望开 + 实际关 + 内核活着 → 静默恢复（外部应用/系统重置）
/// - **残留清理**：实际开 + （期望已关 或 内核已停）→ 静默关闭
///   这一支是"整机断网"的自愈兜底：应用被强杀/崩溃时来不及清理系统代理，
///   重启后期望态（进程内 AtomicBool）归零、主流程又可能走 direct 快速路径，
///   残留的 127.0.0.1 代理就没人收敛了——而此时内核并未运行，整机必然断网。
///
/// 检测/恢复全程静默（不弹提权框），恢复不了下轮再试。
pub fn start_guard(app_handle: tauri::AppHandle) {
    if GUARD_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        interval.tick().await; // 首拍跳过：启动流程自身在设置代理，避免竞态

        loop {
            interval.tick().await;

            let desired = is_desired_enabled();
            let proxy_on = crate::system::sysproxy::get_system_proxy_status();
            if !desired && !proxy_on {
                // 无事可做，也不必惊动内核探测
                RESIDUAL_WARNED.store(false, Ordering::Release);
                continue;
            }

            // 内核存活校验：既是恢复的前置条件，也用于识别"代理在跑但内核没了"
            let running = crate::commands::settings::core_query_running(app_handle.clone())
                .await
                .data
                .unwrap_or(false);

            if desired && !proxy_on {
                // 期望开 + 实际关：内核没跑时属于正常关闭路径（退出流程里），
                // 不做恢复，避免把代理又设回去
                if !running {
                    continue;
                }
                let port = crate::commands::settings::settings_get_internal(&app_handle).mixed_port;
                log::warn!(
                    "[proxy_guard] 检测到系统代理被外部关闭，内核仍在运行，自动恢复 (port {})",
                    port
                );
                if let Err(e) = crate::system::sysproxy::set_system_proxy_silent(true, port) {
                    log::error!("[proxy_guard] 恢复系统代理失败（下轮重试）: {}", e);
                }
            } else if proxy_on && !(desired && running) {
                // 实际开 + 非"期望开且内核活着"：疑似残留，静默关闭
                //
                // 归属闸门：无人值守的自动动作必须先确认"这是我们写的"。
                // Auroweave 只写 127.0.0.1:<mixed_port>，若当前开启的端点不是它，
                // 说明是用户自己的公司代理 / ClashX / Surge —— 绝不干预。
                // 代价是"旧端口残留"会漏清理，但用户点一次开关即收敛，
                // 方向上宁可不作为，也不可误关别人的代理。
                let port = crate::commands::settings::settings_get_internal(&app_handle).mixed_port;
                if !crate::system::sysproxy::is_own_proxy_endpoint(port) {
                    log::debug!("[proxy_guard] 系统代理由外部配置（非本应用写入），不干预");
                    continue;
                }
                let reason = if !desired {
                    "用户期望已关闭"
                } else {
                    "内核已停止"
                };
                let first_hit = !RESIDUAL_WARNED.swap(true, Ordering::AcqRel);
                if first_hit {
                    log::warn!(
                        "[proxy_guard] 检测到残留系统代理（{}），静默关闭以避免整机断网",
                        reason
                    );
                }
                if let Err(e) = crate::system::sysproxy::set_system_proxy_silent(false, 0) {
                    if first_hit {
                        log::error!("[proxy_guard] 清理残留系统代理失败（下轮重试）: {}", e);
                    } else {
                        log::debug!("[proxy_guard] 清理残留系统代理仍失败（下轮重试）: {}", e);
                    }
                }
            } else {
                RESIDUAL_WARNED.store(false, Ordering::Release);
            }
        }
    });
}
