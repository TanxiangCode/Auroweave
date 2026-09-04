/// 系统代理设置 (跨平台)
/// - Windows: 使用原生 Win32 注册表 API
/// - macOS: 使用 networksetup 命令 (通过 osascript 提权)
/// - Linux: 暂未实现
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
            samDeserved: u32,
            phkResult: *mut *mut std::ffi::c_void,
        ) -> i32;

        fn RegQueryValueExW(
            hKey: *mut std::ffi::c_void,
            lpValueName: *const u16,
            lpReserved: *mut u32,
            lpType: *mut u32,
            lpData: *mut u8,
            lpcbData: *mut u32,
        ) -> i32;

        fn RegCloseKey(hKey: *mut std::ffi::c_void) -> i32;
    }

    const HKEY_CURRENT_USER: *mut std::ffi::c_void = 0x80000001 as *mut std::ffi::c_void;
    const KEY_SET_VALUE: u32 = 0x0002;
    const KEY_QUERY_VALUE: u32 = 0x0001;
    const REG_DWORD: u32 = 4;
    const REG_SZ: u32 = 1;

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// 查询注册表 ProxyEnable 是否为 1（系统代理是否开启）
    pub fn is_proxy_enabled() -> bool {
        let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        let mut hkey: *mut std::ffi::c_void = ptr::null_mut();
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
                return false;
            }
            let name = to_wide("ProxyEnable");
            let mut data: u32 = 0;
            let mut cb_data: u32 = 4;
            let res = RegQueryValueExW(hkey, name.as_ptr(), ptr::null_mut(), ptr::null_mut(), &mut data as *mut u32 as *mut u8, &mut cb_data);
            RegCloseKey(hkey);
            res == 0 && data == 1
        }
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

// ===========================================================================
// macOS 系统代理实现 (使用 networksetup 命令)
// ===========================================================================
#[cfg(target_os = "macos")]
mod mac_sysproxy {
    use std::process::Command;

    /// 获取所有已启用的网络服务列表 (Wi-Fi, Ethernet, USB 10/100/1000 LAN 等)
    ///
    /// 通过 `networksetup -listallnetworkservices` 获取，
    /// 第一行是说明文字需跳过，以 `*` 开头的是已禁用的服务也跳过。
    fn get_network_services() -> Vec<String> {
        match Command::new("networksetup")
            .arg("-listallnetworkservices")
            .output()
        {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout
                    .lines()
                    .skip(1) // 第一行: "An asterisk (*) denotes that a network service is disabled."
                    .filter(|line| !line.is_empty() && !line.starts_with('*'))
                    .map(|s| s.trim().to_string())
                    .collect()
            }
            Err(_) => {
                // 回退到常见服务名
                vec!["Wi-Fi".to_string(), "Ethernet".to_string()]
            }
        }
    }

    /// 批量执行 networksetup 命令
    ///
    /// 策略:
    /// 1. 先尝试直接执行 (无 root 权限)，某些 macOS 版本或配置下可能成功
    /// 2. 若直接执行失败且 allow_prompt=true，通过 osascript 提权执行 (会弹出密码框)
    /// 3. 若 allow_prompt=false，直接返回错误
    fn run_networksetup_batch(commands: &[String], allow_prompt: bool) -> Result<(), String> {
        if commands.is_empty() {
            return Ok(());
        }

        // 合并为并发执行的 shell 命令：每个命令后台运行，最后 wait 等待全部完成
        let combined = if commands.len() > 1 {
            format!("{} & wait", commands.join(" & "))
        } else {
            commands.join(" ; ")
        };

        // 1. 先尝试直接执行（无 root 权限）
        let direct_result = Command::new("sh")
            .arg("-c")
            .arg(&combined)
            .output();

        if let Ok(out) = &direct_result {
            if out.status.success() {
                return Ok(());
            }
            // 直接执行失败，记录 stderr 供调试
            let stderr = String::from_utf8_lossy(&out.stderr);
            log::debug!("[sysproxy] networksetup 直接执行失败: {}", stderr.trim());
        }

        if !allow_prompt {
            return Err("networksetup 需要 root 权限，当前为静默模式不提权".to_string());
        }

        // 2. 通过 osascript 提权执行
        // AppleScript do shell script 中需要转义反斜杠和双引号
        let escaped = combined.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            r#"do shell script "{}" with administrator privileges"#,
            escaped
        );

        log::info!("[sysproxy] 通过 osascript 提权执行 networksetup...");
        match Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
        {
            Ok(out) => {
                if out.status.success() {
                    log::info!("[sysproxy] osascript 提权执行成功");
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    Err(format!("networksetup 提权执行失败: {}", stderr.trim()))
                }
            }
            Err(e) => Err(format!("无法执行 osascript: {}", e)),
        }
    }

    /// 查询系统代理是否开启（macOS）
    ///
    /// 通过 `networksetup -getwebproxy Wi-Fi`（或首个可用网络服务）解析
    /// "Enabled: Yes/No" 判断；查询失败按未开启处理。
    pub fn is_proxy_enabled() -> bool {
        let services = get_network_services();
        let service = match services.first() {
            Some(s) => s.replace('\'', "'\\''"),
            None => return false,
        };
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("networksetup -getwebproxy '{}'", service))
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout
                    .lines()
                    .find(|l| l.trim_start().starts_with("Enabled"))
                    .map(|l| l.to_lowercase().contains("yes"))
                    .unwrap_or(false)
            }
            _ => false,
        }
    }

    /// 设置系统代理 (macOS)
    ///
    /// 在所有已启用的网络服务上设置/取消 HTTP、HTTPS、SOCKS 代理。
    ///
    /// 参数:
    /// - enabled: true=开启代理, false=关闭代理
    /// - port: 代理端口 (mixed inbound 端口)
    /// - allow_prompt: 是否允许弹出 macOS 密码框提权 (退出时应设为 false)
    pub fn set_proxy(enabled: bool, port: u16, allow_prompt: bool) -> Result<(), String> {
        let services = get_network_services();
        if services.is_empty() {
            return Err("未找到任何网络服务".to_string());
        }

        let mut commands = Vec::new();
        let port_str = port.to_string();

        for service in &services {
            // 转义服务名中的单引号 (shell 单引号字符串中用 '\'' 转义)
            let s = service.replace('\'', "'\\''");

            if enabled {
                // 设置 HTTP 代理
                commands.push(format!(
                    "networksetup -setwebproxy '{}' 127.0.0.1 {}",
                    s, port_str
                ));
                // 设置 HTTPS 代理
                commands.push(format!(
                    "networksetup -setsecurewebproxy '{}' 127.0.0.1 {}",
                    s, port_str
                ));
                // 设置 SOCKS 代理
                commands.push(format!(
                    "networksetup -setsocksfirewallproxy '{}' 127.0.0.1 {}",
                    s, port_str
                ));
                // 设置代理绕过域名 (localhost 等不走代理)
                commands.push(format!(
                    "networksetup -setproxybypassdomains '{}' 127.0.0.1 localhost '*.local' 169.254/16",
                    s
                ));
            } else {
                // 关闭 HTTP 代理
                commands.push(format!("networksetup -setwebproxystate '{}' off", s));
                // 关闭 HTTPS 代理
                commands.push(format!("networksetup -setsecurewebproxystate '{}' off", s));
                // 关闭 SOCKS 代理
                commands.push(format!("networksetup -setsocksfirewallproxystate '{}' off", s));
            }
        }

        log::info!(
            "[sysproxy] macOS {} 系统代理, 端口: {}, 网络服务数: {}, 允许提权: {}",
            if enabled { "开启" } else { "关闭" },
            port,
            services.len(),
            allow_prompt
        );

        run_networksetup_batch(&commands, allow_prompt)
    }
}

// ===========================================================================
// Linux 系统代理实现 (使用 gsettings 命令对接 GNOME/GTK 桌面环境)
// ===========================================================================
#[cfg(target_os = "linux")]
mod linux_sysproxy {
    use std::process::Command;

    /// 查询 GNOME 系统代理模式是否为 manual（即 Auroweave 开启的代理状态）
    pub fn is_proxy_enabled() -> bool {
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.system.proxy", "mode"])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout).trim().eq_ignore_ascii_case("'manual'")
            }
            _ => false,
        }
    }

    pub fn set_proxy(enabled: bool, port: u16) -> Result<(), String> {
        // 检查系统是否有 gsettings 工具
        let has_gsettings = Command::new("which")
            .arg("gsettings")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !has_gsettings {
            log::warn!("[sysproxy] Linux 环境未检测到 gsettings 命令，跳过系统代理配置");
            return Ok(());
        }

        if enabled {
            log::info!("[sysproxy] Linux 设置 GNOME 系统代理模式为 manual, 端口: {}", port);
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "mode", "'manual'"])
                .status();
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy.http", "host", "'127.0.0.1'"])
                .status();
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy.http", "port", &port.to_string()])
                .status();
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy.https", "host", "'127.0.0.1'"])
                .status();
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy.https", "port", &port.to_string()])
                .status();
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy.socks", "host", "'127.0.0.1'"])
                .status();
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy.socks", "port", &port.to_string()])
                .status();
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "ignore-hosts", "['localhost', '127.0.0.0/8', '::1']"])
                .status();
        } else {
            log::info!("[sysproxy] Linux 设置 GNOME 系统代理模式为 none");
            let _ = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "mode", "'none'"])
                .status();
        }

        Ok(())
    }
}

// ===========================================================================
// 公共 API — 各平台统一接口
// ===========================================================================

/// 查询系统代理当前是否开启
///
/// 供托盘"开→关→开"翻转语义使用：
/// - Windows: 读取注册表 ProxyEnable
/// - macOS: 查询首个网络服务的 Web 代理状态
/// - Linux: 查询 gsettings 代理模式是否为 manual
pub fn get_system_proxy_status() -> bool {
    #[cfg(target_os = "windows")]
    {
        win_registry::is_proxy_enabled()
    }
    #[cfg(target_os = "macos")]
    {
        mac_sysproxy::is_proxy_enabled()
    }
    #[cfg(target_os = "linux")]
    {
        linux_sysproxy::is_proxy_enabled()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

/// 设置系统代理（各平台统一入口）
///
/// 期望状态钩子：成功设置后同步 proxy_guard 期望态——应用内任何路径
/// （托盘/模式切换/自愈）开代理即视为"用户期望开启"，守护开始校验漂移；
/// 关代理即期望关闭，守护立即停手。失败时不更新期望态（守护按旧期望继续）。
pub fn set_system_proxy(enabled: bool, port: u16) -> Result<(), String> {
    let result = set_system_proxy_impl(enabled, port);
    if result.is_ok() {
        crate::system::proxy_guard::set_desired(enabled);
    }
    result
}

/// 静默设置系统代理 (不弹出提权密码框)
///
/// 用于应用退出、启动清理等场景，避免阻塞或打扰用户。
/// 在 macOS 上仅尝试直接执行 networksetup (无 root 时可能失败)；
/// 在 Windows/Linux 上与 set_system_proxy 行为一致 (不需要交互提权)。
/// 期望状态钩子语义与 set_system_proxy 相同（成功才更新期望态）。
pub fn set_system_proxy_silent(enabled: bool, port: u16) -> Result<(), String> {
    let result = set_system_proxy_silent_impl(enabled, port);
    if result.is_ok() {
        crate::system::proxy_guard::set_desired(enabled);
    }
    result
}

#[cfg(target_os = "windows")]
fn set_system_proxy_impl(enabled: bool, port: u16) -> Result<(), String> {
    let server_val = format!("127.0.0.1:{}", port);
    if let Err(e) = win_registry::set_proxy_registry(enabled, &server_val) {
        log::error!("[sysproxy] 写入注册表失败: enabled={}, port={}, 原因: {}", enabled, port, e);
        return Err(e);
    }
    win_sysproxy::refresh();
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_system_proxy_impl(enabled: bool, port: u16) -> Result<(), String> {
    mac_sysproxy::set_proxy(enabled, port, true)
}

#[cfg(target_os = "linux")]
fn set_system_proxy_impl(enabled: bool, port: u16) -> Result<(), String> {
    linux_sysproxy::set_proxy(enabled, port)
}

fn set_system_proxy_silent_impl(enabled: bool, port: u16) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        set_system_proxy_impl(enabled, port)
    }
    #[cfg(target_os = "macos")]
    {
        mac_sysproxy::set_proxy(enabled, port, false)
    }
    #[cfg(target_os = "linux")]
    {
        set_system_proxy_impl(enabled, port)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (enabled, port);
        Ok(())
    }
}
