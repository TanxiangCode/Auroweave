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
/// - 恢复失败（如 networksetup 需提权被用户取消）不重试轰炸：降频为下轮再查
use std::sync::atomic::{AtomicBool, Ordering};

/// 用户期望的系统代理状态（应用内所有写入口同步维护）
static DESIRED_ENABLED: AtomicBool = AtomicBool::new(false);

/// 守护循环是否已启动（防重复 spawn）
static GUARD_STARTED: AtomicBool = AtomicBool::new(false);

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
/// 每 30 秒一拍：期望开 + 实际关 + 内核活着 → 静默恢复。
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

            if !is_desired_enabled() {
                continue;
            }
            if crate::system::sysproxy::get_system_proxy_status() {
                continue; // 实际已开，无漂移
            }

            // 内核存活校验：内核没跑而系统代理被关是"正常关闭路径"，
            // 不做恢复（避免在退出流程里把代理又设回去）
            let running = crate::commands::settings::core_query_running(app_handle.clone())
                .await
                .data
                .unwrap_or(false);
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
        }
    });
}
