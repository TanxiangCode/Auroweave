/// 系统代理设置 (仅在 Windows 生效，其它平台留空)
/// 作者: TanXiang

#[cfg(target_os = "windows")]
mod win_sysproxy {
    use std::ptr;

    #[link(name = "wininet")]
    extern "system" {
        fn InternetSetOptionW(
            hInternet: *mut std::ffi::c_void,
            dwOption: u32,
            lpBuffer: *mut std::ffi::c_void,
            dwBufferLength: u32,
        ) -> i32;
    }

    pub fn refresh() {
        unsafe {
            // INTERNET_OPTION_SETTINGS_CHANGED = 39
            InternetSetOptionW(ptr::null_mut(), 39, ptr::null_mut(), 0);
            // INTERNET_OPTION_REFRESH = 37
            InternetSetOptionW(ptr::null_mut(), 37, ptr::null_mut(), 0);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn set_system_proxy(enabled: bool, port: u16) -> Result<(), String> {
    use std::process::Command;
    let enabled_val = if enabled { "1" } else { "0" };
    let server_val = format!("127.0.0.1:{}", port);

    // 1. 设置 ProxyEnable
    let _ = Command::new("reg")
        .args(&[
            "add",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            "/v",
            "ProxyEnable",
            "/t",
            "REG_DWORD",
            "/d",
            enabled_val,
            "/f",
        ])
        .output()
        .map_err(|e| format!("设置 ProxyEnable 失败: {}", e))?;
    
    if enabled {
        // 2. 设置 ProxyServer
        let _ = Command::new("reg")
            .args(&[
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
                "/v",
                "ProxyServer",
                "/t",
                "REG_SZ",
                "/d",
                &server_val,
                "/f",
            ])
            .output()
            .map_err(|e| format!("设置 ProxyServer 失败: {}", e))?;

        // 3. 设置 ProxyOverride
        let _ = Command::new("reg")
            .args(&[
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
                "/v",
                "ProxyOverride",
                "/t",
                "REG_SZ",
                "/d",
                "localhost;127.0.0.1;<local>",
                "/f",
            ])
            .output()
            .map_err(|e| format!("设置 ProxyOverride 失败: {}", e))?;
    }

    // 4. 刷新设置以立即生效
    win_sysproxy::refresh();

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn set_system_proxy(_enabled: bool, _port: u16) -> Result<(), String> {
    // 暂不支持非 Windows 系统代理设置，可在此预留
    Ok(())
}
