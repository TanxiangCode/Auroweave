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

    /// 读取注册表 ProxyServer 原始值（REG_SZ）
    ///
    /// 形如 "127.0.0.1:8890"，也可能是多协议分机格式
    /// "http=127.0.0.1:8890;https=127.0.0.1:8890"（解析交由上层做）。
    /// 只读不改，用于代理归属判定（见 crate 公共 API 的 is_own_proxy_endpoint）。
    pub fn read_proxy_server() -> Option<String> {
        let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        let mut hkey: *mut std::ffi::c_void = ptr::null_mut();
        unsafe {
            if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_QUERY_VALUE, &mut hkey) != 0 {
                return None;
            }
            let name = to_wide("ProxyServer");
            // 两段式读取：先问长度再取内容（REG_SZ 以 UTF-16 存储）
            let mut cb_data: u32 = 0;
            let size_res = RegQueryValueExW(
                hkey,
                name.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut cb_data,
            );
            if size_res != 0 || cb_data == 0 || cb_data > 4096 {
                RegCloseKey(hkey);
                return None;
            }
            let mut buf = vec![0u16; (cb_data as usize) / 2];
            let read_res = RegQueryValueExW(
                hkey,
                name.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                buf.as_mut_ptr() as *mut u8,
                &mut cb_data,
            );
            RegCloseKey(hkey);
            if read_res != 0 {
                return None;
            }
            let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            let s = String::from_utf16_lossy(&buf[..end]);
            if s.is_empty() { None } else { Some(s) }
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

    /// 批次执行日志路径
    ///
    /// 放 /private/tmp：提权后的 root shell 与普通用户 shell 均可读写，
    /// 且不经过 TCC 保护目录（与 osascript 提权链的 cwd 规避同理）。
    const BATCH_LOG: &str = "/private/tmp/auroweave-networksetup.log";

    /// 状态查询输出的分块标记（前缀 + 序号）
    const QUERY_MARKER: &str = "||AUROWEAVE_PROXY||";

    /// 生成并行执行脚本，并**逐条回收子进程退出码**
    ///
    /// 关键：不能用 `cmd & wait` 收尾——POSIX 里无操作数的 `wait` 退出码恒为 0，
    /// 等于把 networksetup 的每一次失败静默吞掉。旧实现正是如此：macOS 上
    /// 提权被拒或个别网络服务拒绝写入时，调用方依旧收到"成功"，
    /// proxy_guard 期望态被错误改写，残留代理无人收敛（表现为整机断网）。
    fn build_batch_script(commands: &[String]) -> String {
        let mut script = format!(": > {} ; __rc=0", BATCH_LOG);
        for (i, cmd) in commands.iter().enumerate() {
            // `&` 之后紧跟的赋值仍在父 shell 执行，`$!` 才能取到后台子 shell 的 PID
            script.push_str(&format!(
                " ; ( {} ) >> {} 2>&1 & __p{}=$!",
                cmd, BATCH_LOG, i
            ));
        }
        for i in 0..commands.len() {
            // 失败标记只写序号：命令文本可能含引号，翻译成人类可读文本交给 Rust 侧
            script.push_str(&format!(
                " ; wait $__p{0} || {{ __rc=1 ; echo \"AUROWEAVE_FAIL:{0}\" >> {1} ; }}",
                i, BATCH_LOG
            ));
        }
        // 末尾 cat 日志：错误详情随 do shell script 的输出一起回到调用方
        script.push_str(&format!(" ; cat {} ; exit $__rc", BATCH_LOG));
        script
    }

    /// 把批次日志翻译为可定位的失败描述
    fn summarize_batch_failure(log_text: &str, commands: &[String]) -> String {
        let failed: Vec<&String> = commands
            .iter()
            .enumerate()
            .filter(|(i, _)| log_text.contains(&format!("AUROWEAVE_FAIL:{}", i)))
            .map(|(_, cmd)| cmd)
            .collect();
        let output: Vec<&str> = log_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.contains("AUROWEAVE_FAIL:"))
            .collect();

        let mut msg = String::new();
        if !failed.is_empty() {
            msg.push_str(&format!(
                "失败命令: {}",
                failed
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(" | ")
            ));
        }
        if !output.is_empty() {
            let tail: Vec<&str> = output.iter().rev().take(3).rev().copied().collect();
            msg.push_str(&format!("；输出: {}", tail.join(" / ")));
        }
        if msg.is_empty() {
            msg.push_str("无附加输出");
        }
        msg
    }

    /// 批量执行 networksetup 命令
    ///
    /// 策略:
    /// 1. 先尝试直接执行 (无 root 权限)——macOS 上"关闭代理"类命令无需提权，
    ///    常见的开启/关闭往返都在这一步完成
    /// 2. 若直接执行失败且 allow_prompt=true，通过 osascript 提权执行 (会弹出密码框)
    /// 3. 若 allow_prompt=false，直接返回错误
    ///
    /// 返回 Ok 的含义是"**每条命令都成功退出**"，不再有静默吞错的可能。
    fn run_networksetup_batch(commands: &[String], allow_prompt: bool) -> Result<(), String> {
        if commands.is_empty() {
            return Ok(());
        }

        let script = build_batch_script(commands);

        // 1. 先尝试直接执行（无 root 权限）
        match Command::new("sh").arg("-c").arg(&script).output() {
            Ok(out) if out.status.success() => return Ok(()),
            Ok(out) => {
                let detail =
                    summarize_batch_failure(&String::from_utf8_lossy(&out.stdout), commands);
                log::debug!("[sysproxy] networksetup 直接执行失败: {}", detail);
            }
            Err(e) => log::debug!("[sysproxy] networksetup 直接执行无法启动: {}", e),
        }

        if !allow_prompt {
            return Err("networksetup 需要 root 权限，当前为静默模式不提权".to_string());
        }

        // 2. 通过 osascript 提权执行
        // AppleScript do shell script 中需要转义反斜杠和双引号
        let escaped = script.replace('\\', "\\\\").replace('"', "\\\"");
        let apple_script = format!(
            r#"do shell script "{}" with administrator privileges"#,
            escaped
        );

        log::info!("[sysproxy] 通过 osascript 提权执行 networksetup...");
        let mut elevated = Command::new("osascript");
        elevated.arg("-e").arg(&apple_script);
        // CWD 若位于 TCC 受保护目录（~/Documents 等），提权后的 root shell
        // 初始化阶段 getcwd() 即被拒（shell-init 错误），命令必然失败。
        // 显式切换到 /private/tmp 根治（与 sidecar 提权链同款修复）。
        elevated.current_dir("/private/tmp");
        match elevated.output() {
            Ok(out) => {
                if out.status.success() {
                    log::info!("[sysproxy] osascript 提权执行成功");
                    Ok(())
                } else {
                    // do shell script 把内层 stdout（日志）作为结果回传，stderr 里
                    // 则是 "User canceled. (-128)" 之类的 AppleScript 层错误
                    let detail =
                        summarize_batch_failure(&String::from_utf8_lossy(&out.stdout), commands);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let stderr = stderr.trim();
                    if stderr.is_empty() {
                        Err(format!("networksetup 提权执行失败: {}", detail))
                    } else {
                        Err(format!(
                            "networksetup 提权执行失败: {}（osascript: {}）",
                            detail, stderr
                        ))
                    }
                }
            }
            Err(e) => Err(format!("无法执行 osascript: {}", e)),
        }
    }

    /// 单条代理项的查询结果（某服务的 Web / HTTPS / SOCKS 之一）
    #[derive(Debug, PartialEq)]
    struct ProxyEntry {
        /// 与 build_proxy_query 中顺序一致的序号
        index: usize,
        enabled: bool,
        server: String,
        port: u16,
    }

    /// 生成一次性状态查询脚本：每个已启用服务的 Web / HTTPS / SOCKS 三项
    ///
    /// 每项前置 `||AUROWEAVE_PROXY||<i>` 标记：`-getsecurewebproxy` 的输出
    /// 比 `-getwebproxy` 多一行认证信息，靠分隔标记切块比按行数切更稳。
    /// 合成一条 shell 命令执行，避免逐条 spawn（守护每 30s 一拍，开销敏感）。
    fn build_proxy_query(services: &[String]) -> String {
        let mut parts: Vec<String> = Vec::new();
        for service in services {
            // 转义服务名中的单引号 (shell 单引号字符串中用 '\'' 转义)
            let s = service.replace('\'', "'\\''");
            for kind in ["web", "secureweb", "socksfirewall"] {
                let marker = format!("{}{}", QUERY_MARKER, parts.len());
                parts.push(format!(
                    "echo '{}' ; networksetup -get{}proxy '{}'",
                    marker, kind, s
                ));
            }
        }
        parts.join(" ; ")
    }

    /// 解析 networksetup 查询输出
    ///
    /// 注意必须"任一"而非"首个"：多网卡（Wi-Fi + 有线 + Tailscale/雷雳桥）
    /// 场景下，首个服务已关而其余仍开着同样会断网。
    fn parse_proxy_entries(stdout: &str) -> Vec<ProxyEntry> {
        let mut entries: Vec<ProxyEntry> = Vec::new();
        let mut current: Option<ProxyEntry> = None;
        for line in stdout.lines() {
            let t = line.trim();
            if let Some(idx) = t.strip_prefix(QUERY_MARKER) {
                if let Some(done) = current.take() {
                    entries.push(done);
                }
                current = Some(ProxyEntry {
                    index: idx.trim().parse().unwrap_or(usize::MAX),
                    enabled: false,
                    server: String::new(),
                    port: 0,
                });
                continue;
            }
            // 分隔标记之前的内容一律忽略（查询失败时的错误输出等）
            let Some(entry) = current.as_mut() else {
                continue;
            };
            if let Some(v) = t.strip_prefix("Enabled:") {
                // "Authenticated Proxy Enabled: 0" 不以 Enabled: 开头，天然被排除
                entry.enabled = v.trim().eq_ignore_ascii_case("yes");
            } else if let Some(v) = t.strip_prefix("Server:") {
                entry.server = v.trim().to_string();
            } else if let Some(v) = t.strip_prefix("Port:") {
                entry.port = v.trim().parse().unwrap_or(0);
            }
        }
        if let Some(done) = current.take() {
            entries.push(done);
        }
        entries
    }

    /// 查询全部已启用服务的三项代理状态
    fn query_proxy_entries() -> Vec<ProxyEntry> {
        let services = get_network_services();
        if services.is_empty() {
            return Vec::new();
        }
        match Command::new("sh")
            .arg("-c")
            .arg(build_proxy_query(&services))
            .output()
        {
            Ok(out) => parse_proxy_entries(&String::from_utf8_lossy(&out.stdout)),
            Err(e) => {
                log::debug!("[sysproxy] 查询系统代理状态失败: {}", e);
                Vec::new()
            }
        }
    }

    /// 查询系统代理是否开启（macOS）
    ///
    /// 遍历**所有**已启用的网络服务，Web/HTTPS/SOCKS 任一为 "Enabled: Yes"
    /// 即视为开启；单个服务查询失败（服务已消失等）按该项未开启处理。
    ///
    /// 旧实现只看 `services.first()`，会把"首服务已关、其余仍开着"误判为
    /// 已关闭——托盘显示已关闭、守护认为无漂移，残留代理持续生效导致整机
    /// 断网而无人察觉。
    pub fn is_proxy_enabled() -> bool {
        query_proxy_entries().iter().any(|e| e.enabled)
    }

    /// 当前开启项指向的端点（server, port），未开启返回 None
    pub fn enabled_endpoint() -> Option<(String, u16)> {
        query_proxy_entries()
            .into_iter()
            .find(|e| e.enabled)
            .filter(|e| !e.server.is_empty() && e.port > 0)
            .map(|e| (e.server, e.port))
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

    #[cfg(test)]
    mod tests {
        use super::{
            build_batch_script, build_proxy_query, parse_proxy_entries, run_networksetup_batch,
            summarize_batch_failure, BATCH_LOG, QUERY_MARKER,
        };

        #[test]
        fn batch_succeeds_when_all_commands_exit_zero() {
            // 真跑一次 sh：全成功批次必须 Ok（命令本身无害，不触碰网络设置）
            run_networksetup_batch(&["true".to_string(), "echo ok".to_string()], false)
                .expect("全部命令零退出码时应成功");
        }

        #[test]
        fn batch_reports_failure_instead_of_silent_success() {
            // 核心回归防线：旧实现下这条恒为 Ok（`cmd & wait` 吞掉退出码），
            // 上层据此误判"代理已关闭"，残留 127.0.0.1 代理 → 整机断网
            let err = run_networksetup_batch(
                &["sh -c 'echo ** Error: nope >&2; exit 3'".to_string()],
                false,
            )
            .expect_err("失败命令必须上报错误");
            // 静默模式不提权，因此止步于权限分支，但错误必须真实返回
            assert!(err.contains("root 权限"), "{err}");
        }

        #[test]
        fn batch_script_never_uses_bare_wait() {
            // 回归防线：`cmd & wait` 中无操作数的 wait 退出码恒为 0，
            // 会把 networksetup 的失败全部吞掉（残留代理 → 整机断网）
            let commands = vec!["networksetup -setwebproxystate 'Wi-Fi' off".to_string()];
            let script = build_batch_script(&commands);
            assert!(
                !script.contains("& wait"),
                "批次脚本不得使用裸 wait: {script}"
            );
            assert!(script.contains("wait $__p0 ||"), "缺少逐条回收: {script}");
            assert!(script.ends_with("exit $__rc"), "{script}");
        }

        #[test]
        fn batch_script_collects_exit_code_of_every_command() {
            let commands = vec![
                "networksetup -setwebproxystate 'Wi-Fi' off".to_string(),
                "networksetup -setwebproxystate 'Tailscale' off".to_string(),
            ];
            let script = build_batch_script(&commands);
            for i in 0..commands.len() {
                assert!(script.contains(&format!("__p{i}=$!")), "{script}");
                assert!(script.contains(&format!("wait $__p{i} ||")), "{script}");
                assert!(script.contains(&format!("AUROWEAVE_FAIL:{i}")), "{script}");
            }
            assert!(script.contains(BATCH_LOG), "批次日志路径缺失: {script}");
        }

        #[test]
        fn batch_script_is_single_line_for_osascript_escaping() {
            // 提权路径把整段脚本塞进 AppleScript 字符串字面量，跨行会破坏字面量
            let commands = vec!["networksetup -listallnetworkservices".to_string()];
            let script = build_batch_script(&commands);
            assert!(!script.contains('\n'), "脚本必须保持单行: {script}");
        }

        #[test]
        fn parse_proxy_entries_detects_any_service_left_on() {
            // 模拟多网卡：Wi-Fi/有线已关，雷雳桥仍开着（-getsecurewebproxy 多一行认证信息）
            let stdout = "||AUROWEAVE_PROXY||0\nEnabled: No\nServer: 127.0.0.1\nPort: 8890\n\
                          ||AUROWEAVE_PROXY||1\nEnabled: No\nServer: 127.0.0.1\nPort: 8890\n\
                          Authenticated Proxy Enabled: 0\n\
                          ||AUROWEAVE_PROXY||2\nEnabled: Yes\nServer: 127.0.0.1\nPort: 8890\n";
            let entries = parse_proxy_entries(stdout);
            assert_eq!(entries.len(), 3);
            assert!(!entries[0].enabled && !entries[1].enabled);
            assert!(entries[2].enabled);
            assert!(entries.iter().any(|e| e.enabled));
        }

        #[test]
        fn parse_proxy_entries_is_all_off_when_everything_off() {
            let stdout = "||AUROWEAVE_PROXY||0\nEnabled: No\nServer: 127.0.0.1\nPort: 8890\n\
                          ||AUROWEAVE_PROXY||1\nEnabled: No\nServer: 127.0.0.1\nPort: 8890\n";
            let entries = parse_proxy_entries(stdout);
            assert!(!entries.iter().any(|e| e.enabled));
            assert!(parse_proxy_entries("").is_empty());
        }

        #[test]
        fn parse_proxy_entries_keeps_endpoint_of_enabled_item() {
            // 归属判定的数据来源：开启项的 Server/Port 必须被正确取到
            let stdout = "||AUROWEAVE_PROXY||0\nEnabled: No\nServer: 10.0.0.1\nPort: 8080\n\
                          ||AUROWEAVE_PROXY||1\nEnabled: Yes\nServer: 127.0.0.1\nPort: 8890\n";
            let entries = parse_proxy_entries(stdout);
            let hit = entries.iter().find(|e| e.enabled).expect("应有开启项");
            assert_eq!((hit.server.as_str(), hit.port), ("127.0.0.1", 8890));
        }

        #[test]
        fn proxy_query_script_covers_all_three_proxy_kinds() {
            let services = vec!["Wi-Fi".to_string(), "Thunderbolt Bridge".to_string()];
            let script = build_proxy_query(&services);
            for service in &services {
                assert!(
                    script.contains(&format!("-getwebproxy '{service}'")),
                    "{script}"
                );
                assert!(
                    script.contains(&format!("-getsecurewebproxy '{service}'")),
                    "{script}"
                );
                assert!(
                    script.contains(&format!("-getsocksfirewallproxy '{service}'")),
                    "{script}"
                );
            }
            // 每个服务三项 = 6 个分块标记，解析才切得开
            assert_eq!(script.matches(QUERY_MARKER).count(), 6, "{script}");
        }

        #[test]
        fn summarize_failure_names_the_failing_command() {
            let commands = vec![
                "networksetup -setwebproxystate 'Wi-Fi' off".to_string(),
                "networksetup -setwebproxystate 'Tailscale' off".to_string(),
            ];
            let log = "AUROWEAVE_FAIL:1\n** Error: The parameters were not valid.";
            let summary = summarize_batch_failure(log, &commands);
            assert!(summary.contains("Tailscale"), "{summary}");
            assert!(!summary.contains("'Wi-Fi'"), "{summary}");
            assert!(summary.contains("parameters were not valid"), "{summary}");
        }
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
// 代理归属判定（跨平台共用）
// ===========================================================================

/// 判断主机是否为环回地址（IPv4/IPv6/localhost，兼容 Windows 的 "[::1]" 写法）
///
/// 本文件所有平台实现写入的代理服务器都是 127.0.0.1，因此"环回"是
/// "这是本应用留下的"的第一道判据。
fn is_loopback_host(host: &str) -> bool {
    let h = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_matches(|c| c == ' ' || c == '"' || c == '\'')
        .to_ascii_lowercase();
    h == "localhost" || h == "::1" || h == "0:0:0:0:0:0:0:1" || h.starts_with("127.")
}

/// 解析代理服务器字符串为 (host, port)
///
/// 兼容 Windows 两种常见格式：
/// - 单一端点："127.0.0.1:8890"
/// - 多协议分机："http=127.0.0.1:8890;https=127.0.0.1:8890"（取第一个分机）
///
/// 端口缺失/非法时返回 None（调用方按"无法归属"处理，绝不误判）。
// 非 Windows 平台没有调用方（端点来自 networksetup 输出），但保留为跨平台纯
// 函数以便在任意开发机上单测这条判据。
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_proxy_server(raw: &str) -> Option<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let first = raw.split(';').next().unwrap_or(raw);
    // 形如 "http=host:port"：取等号右侧
    let value = match first.split_once('=') {
        Some((_proto, v)) => v,
        None => first,
    }
    .trim();
    let (host, port) = value.rsplit_once(':')?;
    let port: u16 = port.trim().parse().ok()?;
    let host = host.trim();
    if host.is_empty() || port == 0 {
        return None;
    }
    Some((host.to_string(), port))
}

/// 查询当前开启的系统代理指向的端点（host, port）；未开启返回 None
pub fn get_system_proxy_endpoint() -> Option<(String, u16)> {
    #[cfg(target_os = "windows")]
    {
        if !win_registry::is_proxy_enabled() {
            return None;
        }
        win_registry::read_proxy_server()
            .as_deref()
            .and_then(parse_proxy_server)
    }
    #[cfg(target_os = "macos")]
    {
        mac_sysproxy::enabled_endpoint()
    }
    // Linux/其他：端点读取未实现（gsettings 键路径差异大），一律按"无法归属"
    // 处理 —— 守护因此不自动干预，宁可不清理也绝不误关用户代理。
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        None
    }
}

/// 判断当前开启的系统代理是否为**本应用写入**的残留
///
/// 判据：环回地址 + 我们管理的端口（`settings.mixed_port`）——本文件所有平台
/// 实现都只写 `127.0.0.1:<mixed_port>`，因此"环回 + 端口命中"即可确定归属。
///
/// 为什么必须有这道闸：守护的"残留清理"是**无人值守**的自动动作，不看归属
/// 就关闭，会把用户自己配置的公司代理 / ClashX / Surge 代理静默关掉，且应用内
/// 没有恢复入口。宁可漏清理（用户点一次开关即收敛），不可误关。
pub fn is_own_proxy_endpoint(managed_port: u16) -> bool {
    match get_system_proxy_endpoint() {
        Some((host, port)) => port == managed_port && is_loopback_host(&host),
        None => false,
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

/// 确保系统代理处于关闭态（"关闭代理"动作的成功判据）
///
/// 为什么必须回读校验：系统代理指向 127.0.0.1 而内核已停 = **整机断网**。
/// 只要"关闭"没真正落到所有网络服务上，用户就彻底失去网络且毫无提示，
/// 因此这里的返回值必须代表"回读确认已关闭"，而不是"写入命令发起过"：
/// - 回读已关闭 → 直接返回，零开销零提权（绝大多数情况）
/// - 回读仍开启 → 带提权重试（用户显式点了关闭，允许弹一次密码框）
/// - 仍关不掉 → 返回 Err，由上层兜底（拉起内核）并向用户报错
///
/// 期望态无论成败都先置 false：用户的意图是"关"，守护不应再把它打开
/// （否则下次 30s 拍的漂移恢复会把刚残留的代理又拉起来）。
pub fn ensure_system_proxy_disabled() -> Result<(), String> {
    crate::system::proxy_guard::set_desired(false);

    if !get_system_proxy_status() {
        return Ok(());
    }

    log::info!("[sysproxy] 回读发现系统代理仍处于开启态，执行关闭（含提权重试）");
    // 直接用 impl 而非 set_system_proxy：期望态已在上面显式置 false，
    // 避免"写入失败但钩子按成功处理"的语义混淆
    set_system_proxy_impl(false, 0)?;

    if get_system_proxy_status() {
        return Err("系统代理仍处于开启状态（部分网络服务拒绝关闭），整机可能断网，请检查 系统设置 → 网络 → 详细信息 → 代理".to_string());
    }
    Ok(())
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

/// 刷新操作系统本地 DNS 解析缓存（环境自洁）
/// - macOS: 执行 dscacheutil -flushcache（普通用户权限即可，清除残留 Fake-IP 映射，杜绝 15 秒假死）
/// - Windows: 执行 ipconfig /flushdns
/// - Linux: 若使用 systemd-resolved 则尝试 resolvectl flush-caches
pub fn flush_system_dns_cache() {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("dscacheutil")
            .arg("-flushcache")
            .output();
        log::debug!("[sysproxy] macOS 本地 DNS 缓存已刷新 (dscacheutil -flushcache)");
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::process::Command::new("ipconfig")
            .arg("/flushdns")
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        log::debug!("[sysproxy] Windows 本地 DNS 缓存已刷新 (ipconfig /flushdns)");
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("resolvectl")
            .arg("flush-caches")
            .output();
    }
}

/// 代理归属判定的跨平台单测（不依赖具体平台的系统调用）
#[cfg(test)]
mod attribution_tests {
    use super::{is_loopback_host, parse_proxy_server};

    #[test]
    fn loopback_detection_covers_all_local_forms() {
        for host in [
            "127.0.0.1",
            "127.0.0.53",
            "localhost",
            "LOCALHOST",
            "::1",
            "[::1]",
            "0:0:0:0:0:0:0:1",
            " 127.0.0.1 ",
        ] {
            assert!(is_loopback_host(host), "应识别为环回: {host}");
        }
    }

    #[test]
    fn non_loopback_hosts_are_rejected() {
        // 守护的归属闸门：公司代理 / ClashX 的远程端点绝不能被当成"我们写的"
        for host in ["10.0.0.1", "proxy.corp.example.com", "192.168.1.10", ""] {
            assert!(!is_loopback_host(host), "不应视为环回: {host}");
        }
    }

    #[test]
    fn proxy_server_parsing_handles_windows_both_formats() {
        assert_eq!(
            parse_proxy_server("127.0.0.1:8890"),
            Some(("127.0.0.1".to_string(), 8890))
        );
        assert_eq!(
            parse_proxy_server("http=127.0.0.1:8890;https=127.0.0.1:8890"),
            Some(("127.0.0.1".to_string(), 8890))
        );
        assert_eq!(
            parse_proxy_server("socks=proxy.corp.example.com:8080"),
            Some(("proxy.corp.example.com".to_string(), 8080))
        );
    }

    #[test]
    fn proxy_server_parsing_rejects_malformed_values() {
        // 解析不出来 → 无法归属 → 守护不干预（安全侧）
        for raw in [
            "",
            "   ",
            "127.0.0.1",
            "127.0.0.1:abc",
            "127.0.0.1:0",
            ":8890",
        ] {
            assert!(parse_proxy_server(raw).is_none(), "不应解析成功: {raw:?}");
        }
    }
}
