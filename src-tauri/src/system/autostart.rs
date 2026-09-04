/// 开机自启管理（tauri-plugin-autostart 封装）
/// 作者: TanXiang
///
/// 职责：把 settings.auto_start 设置项同步为系统级自启注册。
/// macOS 使用 LaunchAgent（login item），无需提权；
/// 插件初始化参数在 lib.rs 中统一配置（MacosLauncher::LaunchAgent）。
use tauri_plugin_autostart::ManagerExt;

/// 将 auto_start 设置同步到系统自启注册。
/// 调用时机：setup 启动阶段 + settings_save 检测到字段变化后。
/// 幂等：enable 对已注册项、disable 对未注册项均为 no-op。
pub fn sync_autostart(app_handle: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let autostart = app_handle.autolaunch();
    let current = autostart
        .is_enabled()
        .map_err(|e| format!("读取自启注册状态失败: {}", e))?;

    if enabled == current {
        return Ok(());
    }

    if enabled {
        autostart
            .enable()
            .map_err(|e| format!("开启开机自启失败: {}", e))?;
        log::info!("[autostart] 已注册开机自启（login item）");
    } else {
        autostart
            .disable()
            .map_err(|e| format!("关闭开机自启失败: {}", e))?;
        log::info!("[autostart] 已注销开机自启");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // sync_autostart 依赖 AppHandle（Tauri 运行时），无法脱离应用做单元测试；
    // 插件本身的 enable/disable 幂等性由 tauri-plugin-autostart 保证。
    // 此模块通过运行时手测验证：设置开关切换后检查
    // ~/Library/LaunchAgents/ 下 plist 的存在性。
}
