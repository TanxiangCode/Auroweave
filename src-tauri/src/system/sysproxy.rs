/// 系统代理设置 (仅在 Windows 生效，使用原生 Win32 注册表 API)
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
mod win_registry {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            hKey: *mut std::ffi::c_void,
            lpSubKey: *const u16,
            ulOptions: u32,
            samDesired: u32,
            phkResult: *mut *mut std::ffi::c_void,
        ) -> i32;

        fn RegSetValueExW(
            hKey: *mut std::ffi::c_void,
            lpValueName: *const u16,
            Reserved: u32,
            dwType: u32,
            lpData: *const u8,
            cbData: u32,
        ) -> i32;

        fn RegCloseKey(hKey: *mut std::ffi::c_void) -> i32;
    }

    const HKEY_CURRENT_USER: *mut std::ffi::c_void = 0x80000001 as *mut std::ffi::c_void;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_DWORD: u32 = 4;
    const REG_SZ: u32 = 1;

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn set_proxy_registry(enabled: bool, server: &str) -> Result<(), String> {
        let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        let mut hkey: *mut std::ffi::c_void = ptr::null_mut();

        unsafe {
            let res = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            );
            if res != 0 {
                return Err(format!("无法打开注册表键，错误码: {}", res));
            }

            // 1. 设置 ProxyEnable
            let enable_name = to_wide("ProxyEnable");
            let enable_val: u32 = if enabled { 1 } else { 0 };
            let res = RegSetValueExW(
                hkey,
                enable_name.as_ptr(),
                0,
                REG_DWORD,
                &enable_val as *const u32 as *const u8,
                4,
            );
            if res != 0 {
                RegCloseKey(hkey);
                return Err(format!("写入 ProxyEnable 失败，错误码: {}", res));
            }

            if enabled {
                // 2. 设置 ProxyServer
                let server_name = to_wide("ProxyServer");
                let server_wide = to_wide(server);
                let res = RegSetValueExW(
                    hkey,
                    server_name.as_ptr(),
                    0,
                    REG_SZ,
                    server_wide.as_ptr() as *const u8,
                    (server_wide.len() * 2) as u32,
                );
                if res != 0 {
                    RegCloseKey(hkey);
                    return Err(format!("写入 ProxyServer 失败，错误码: {}", res));
                }

                // 3. 设置 ProxyOverride
                let override_name = to_wide("ProxyOverride");
                let override_wide = to_wide("localhost;127.0.0.1;<local>");
                let res = RegSetValueExW(
                    hkey,
                    override_name.as_ptr(),
                    0,
                    REG_SZ,
                    override_wide.as_ptr() as *const u8,
                    (override_wide.len() * 2) as u32,
                );
                if res != 0 {
                    RegCloseKey(hkey);
                    return Err(format!("写入 ProxyOverride 失败，错误码: {}", res));
                }
            }

            RegCloseKey(hkey);
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
pub fn set_system_proxy(enabled: bool, port: u16) -> Result<(), String> {
    let server_val = format!("127.0.0.1:{}", port);
    win_registry::set_proxy_registry(enabled, &server_val)?;
    win_sysproxy::refresh();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn set_system_proxy(_enabled: bool, _port: u16) -> Result<(), String> {
    Ok(())
}
