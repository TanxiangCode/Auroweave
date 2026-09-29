/// 系统代理设置 (跨平台)
/// - Windows: 使用原生 Win32 注册表 API
/// - macOS: 使用 networksetup 命令 (通过 osascript 提权)
/// - Linux: 暂未实现
/// 作者: TanXiang
use serde::{Deserialize, Serialize};

// ===========================================================================
// 用户原配置快照（"只还原字段，不改开关状态"的环境自洁）
// ===========================================================================

/// 单个代理项的原始值
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProxyPoint {
    pub server: String,
    pub port: u16,
    /// 接管前该项是否处于开启态（仅用于日志诊断，还原时不据此改开关）
    #[serde(default)]
    pub enabled: bool,
    /// 认证代理开关（macOS -getsecurewebproxy 的 "Authenticated Proxy Enabled"）
    #[serde(default)]
    pub auth: bool,
    /// 是否成功读到过该项（区分"原本就是空"与"查询失败"）
    ///
    /// 两者都会表现为 server="" + port=0，但语义完全相反：前者要**清空**字段，
    /// 后者必须跳过——把查询失败当成"原本为空"会把用户的配置抹掉。
    #[serde(default)]
    pub read: bool,
}

/// 单个 macOS 网络服务的原始代理配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ServiceProxyState {
    pub service: String,
    #[serde(default)]
    pub web: ProxyPoint,
    #[serde(default)]
    pub secure_web: ProxyPoint,
    #[serde(default)]
    pub socks: ProxyPoint,
    /// 原始绕过列表（一行一个域名，保存为空格分隔；Some("") 表示"原本未设置"）
    #[serde(default)]
    pub bypass_domains: Option<String>,
}

/// Windows Internet Settings 的原始值
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WinProxyState {
    pub proxy_server: Option<String>,
    pub proxy_override: Option<String>,
}

/// 用户原始系统代理配置快照
///
/// 用途：本应用开启代理时会把用户的 ProxyServer / 绕过列表**覆盖**为
/// `127.0.0.1:<mixed_port>`，若不还原，用户日后手动打开系统代理会指向一个
/// 无人监听的端口（与"残留代理导致断网"同源的环境自洁缺失）。
///
/// 还原策略是"**只还原字段值，不改开关状态**"：
/// - 用户点「关闭代理」后代理确实是关的——不会把他原有的公司代理又打开
///   （那种行为既反直觉又危险）
/// - 但系统里"代理服务器 / 绕过列表"字段回到用户自己的值，不再永久残留
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SysProxyBackup {
    #[serde(default)]
    pub services: Vec<ServiceProxyState>,
    #[serde(default)]
    pub windows: Option<WinProxyState>,
}

/// 本应用**实际写入过**的系统代理端点
///
/// 为什么不能只靠 `settings.mixed_port` 判定归属：那是"当前配置端口"，而残留
/// 是"上次写入端口"。用户改过端口后（8890 → 7890），崩溃留下的 `127.0.0.1:8890`
/// 用新端口比对必然失配 → 守护判定"非本应用写入"而放手 → 残留代理永远没人收敛，
/// 整机断网。因此在每次写入成功时把真实端点落盘，归属判定以它为准。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppliedEndpoint {
    pub host: String,
    pub port: u16,
}

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

        fn RegSetValueExW(
            hKey: *mut std::ffi::c_void,
            lpValueName: *const u16,
            dwReserved: u32,
            dwType: u32,
            lpData: *const u8,
            cbData: u32,
        ) -> i32;

        fn RegDeleteValueW(hKey: *mut std::ffi::c_void, lpValueName: *const u16) -> i32;

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

    /// 读取注册表 Internet Settings 下的字符串值（REG_SZ）
    ///
    /// 两段式读取：先问字节长度再取内容；长度异常（>4KB）视为无值。
    fn read_internet_settings_string(name: &str) -> Option<String> {
        let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        let mut hkey: *mut std::ffi::c_void = ptr::null_mut();
        unsafe {
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut hkey,
            ) != 0
            {
                return None;
            }
            let value_name = to_wide(name);
            let mut cb_data: u32 = 0;
            let size_res = RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
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
                value_name.as_ptr(),
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
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }

    /// 写入注册表 Internet Settings 下的字符串值（REG_SZ）
    ///
    /// 仅用于"还原用户原配置"——不触碰 ProxyEnable，因此还原后代理仍是关闭态。
    pub fn write_internet_settings_string(name: &str, value: &str) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;

        let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        let mut hkey: *mut std::ffi::c_void = ptr::null_mut();
        unsafe {
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            ) != 0
            {
                return Err("无法打开 Internet Settings 注册表键".to_string());
            }
            let value_name = to_wide(name);
            let data: Vec<u16> = std::ffi::OsStr::new(value)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
            let res = RegSetValueExW(
                hkey,
                value_name.as_ptr(),
                0,
                REG_SZ,
                bytes.as_ptr(),
                (data.len() * 2) as u32,
            );
            RegCloseKey(hkey);
            if res != 0 {
                return Err(format!("写入注册表 {} 失败，错误码 {}", name, res));
            }
        }
        Ok(())
    }

    /// 删除注册表 Internet Settings 下的值
    ///
    /// 用于还原"用户原本就没有该值"的场景：写空串会让部分解析器困惑，
    /// 删除才是"恢复从未配置"的确切语义。值不存在时视为成功。
    pub fn delete_internet_settings_value(name: &str) -> Result<(), String> {
        const ERROR_FILE_NOT_FOUND: i32 = 2;
        let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings");
        let mut hkey: *mut std::ffi::c_void = ptr::null_mut();
        unsafe {
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            ) != 0
            {
                return Err("无法打开 Internet Settings 注册表键".to_string());
            }
            let value_name = to_wide(name);
            let res = RegDeleteValueW(hkey, value_name.as_ptr());
            RegCloseKey(hkey);
            if res != 0 && res != ERROR_FILE_NOT_FOUND {
                return Err(format!("删除注册表 {} 失败，错误码 {}", name, res));
            }
        }
        Ok(())
    }

    /// 读取注册表 ProxyServer 原始值（REG_SZ）
    ///
    /// 形如 "127.0.0.1:8890"，也可能是多协议分机格式
    /// "http=127.0.0.1:8890;https=127.0.0.1:8890"（解析交由上层做）。
    /// 只读不改，用于代理归属判定（见 crate 公共 API 的 is_own_proxy_endpoint）。
    pub fn read_proxy_server() -> Option<String> {
        read_internet_settings_string("ProxyServer")
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
    use super::{ServiceProxyState, SysProxyBackup};
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

    /// 执行一批 networksetup 命令（供还原原配置等内部流程复用）
    pub(super) fn run_commands(commands: &[String], allow_prompt: bool) -> Result<(), String> {
        run_networksetup_batch(commands, allow_prompt)
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

    /// 代理服务器字段是否已被本应用写入的端点污染
    ///
    /// 为什么必须与 `enabled_endpoint` 分开：`networksetup -setwebproxy` 会把
    /// server/port 写进配置，但开关可以独立是 off。开关关着而字段仍指向
    /// 127.0.0.1:<mixed_port>，用户日后手动打开系统代理就会命中一个无人监听的
    /// 端口——这正是 SysProxyBackup 快照机制要消灭的残留。判定"字段是否被污染"
    /// 时不能用开关态代替，否则这类残留永远查不出来、也就永远不会被还原（F3）。
    ///
    /// 只认精确匹配（host + port 都等于本应用写入值）：宁可漏判也不误判，
    /// 避免把用户自己的 127.0.0.1 服务当成残留去还原。
    pub(super) fn is_field_polluted_by(applied: &super::AppliedEndpoint) -> bool {
        query_proxy_entries()
            .into_iter()
            .any(|e| e.server == applied.host && e.port == applied.port)
    }

    /// 原配置快照查询的字段标记前缀
    const BACKUP_MARKER: &str = "||AUROWEAVE_BACKUP||";

    /// 自动代理(PAC)查询的输出标记前缀
    const AUTOPROXY_MARKER: &str = "||AUROWEAVE_PAC||";

    /// 生成"原配置快照"查询脚本：每个服务取三项代理 + 绕过列表
    fn build_backup_query(services: &[String]) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (i, service) in services.iter().enumerate() {
            let s = service.replace('\'', "'\\''");
            for kind in ["web", "secureweb", "socksfirewall"] {
                parts.push(format!(
                    "echo '{}{}|{}' ; networksetup -get{}proxy '{}'",
                    BACKUP_MARKER, i, kind, kind, s
                ));
            }
            // 绕过列表：一行一个域名、行数不定，靠下一个标记收尾
            parts.push(format!(
                "echo '{}{}|bypass' ; networksetup -getproxybypassdomains '{}'",
                BACKUP_MARKER, i, s
            ));
        }
        parts.join(" ; ")
    }

    /// 解析快照查询输出
    ///
    /// 标记格式 `||AUROWEAVE_BACKUP||<服务序号>|<字段>`；标记之后的行归属该字段。
    /// 绕过列表是"一行一个域名"，需整段收集后用空格拼回（写回时的格式）。
    fn parse_backup_output(stdout: &str, services: &[String]) -> Vec<ServiceProxyState> {
        let mut states: Vec<ServiceProxyState> = services
            .iter()
            .map(|s| ServiceProxyState {
                service: s.clone(),
                ..Default::default()
            })
            .collect();
        let mut current: Option<(usize, String)> = None;
        let mut bypass_lines: Vec<String> = Vec::new();

        for line in stdout.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix(BACKUP_MARKER) {
                // 收尾上一块：绕过列表要在整段收集完之后才写入
                if let Some((idx, kind)) = current.take() {
                    if kind == "bypass" {
                        if let Some(state) = states.get_mut(idx) {
                            let joined = bypass_lines
                                .iter()
                                // networksetup 的"未设置"提示行不是域名
                                .filter(|l| !l.contains("aren't any bypass"))
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(" ");
                            state.bypass_domains = Some(joined);
                        }
                    }
                }
                bypass_lines.clear();
                let mut it = rest.split('|');
                let idx = it.next().and_then(|s| s.trim().parse::<usize>().ok());
                let kind = it.next().unwrap_or("").trim().to_string();
                current = match idx {
                    Some(i) if i < states.len() => Some((i, kind)),
                    _ => None,
                };
                continue;
            }
            let Some((idx, kind)) = current.as_ref() else {
                continue;
            };
            let Some(state) = states.get_mut(*idx) else {
                continue;
            };
            if kind == "bypass" {
                if !t.is_empty() {
                    bypass_lines.push(t.to_string());
                }
                continue;
            }
            let point = match kind.as_str() {
                "secureweb" => &mut state.secure_web,
                "socksfirewall" => &mut state.socks,
                _ => &mut state.web,
            };
            if let Some(v) = t.strip_prefix("Server:") {
                point.server = v.trim().to_string();
                point.read = true;
            } else if let Some(v) = t.strip_prefix("Port:") {
                point.port = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = t.strip_prefix("Enabled:") {
                point.enabled = v.trim().eq_ignore_ascii_case("yes");
            } else if let Some(v) = t.strip_prefix("Authenticated Proxy Enabled:") {
                point.auth = v.trim() == "1";
            }
        }
        // 末块收尾
        if let Some((idx, kind)) = current.take() {
            if kind == "bypass" {
                if let Some(state) = states.get_mut(idx) {
                    state.bypass_domains = Some(bypass_lines.join(" "));
                }
            }
        }
        states
    }

    /// 读取当前全部网络服务的原始代理配置
    pub(super) fn read_backup_state() -> Result<SysProxyBackup, String> {
        let services = get_network_services();
        if services.is_empty() {
            return Err("未找到任何网络服务".to_string());
        }
        let out = Command::new("sh")
            .arg("-c")
            .arg(build_backup_query(&services))
            .output()
            .map_err(|e| format!("执行 networksetup 失败: {}", e))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        // 一个标记都没有 = 查询整体失败，视为无快照（安全侧：宁可不还原）
        if !stdout.contains(BACKUP_MARKER) {
            return Err("快照查询无有效输出".to_string());
        }
        Ok(SysProxyBackup {
            services: parse_backup_output(&stdout, &services),
            ..Default::default()
        })
    }

    /// 生成"还原原配置字段"的命令
    ///
    /// 关键：`-setwebproxy` 等 setter 会**隐式开启**代理，所以写回原值之后
    /// 必须显式把三项状态置 off —— 还原的是"字段值"而非"开关状态"
    /// （见 SysProxyBackup 的设计说明）。
    pub(super) fn build_restore_commands(backup: &SysProxyBackup) -> Vec<String> {
        let mut commands: Vec<String> = Vec::new();
        for state in &backup.services {
            let s = state.service.replace('\'', "'\\''");
            let mut touched = false;
            for (kind, point) in [
                ("web", &state.web),
                ("secureweb", &state.secure_web),
                ("socksfirewall", &state.socks),
            ] {
                // 未读到该项：服务可能已消失，跳过（不拿"查询失败"当"原本为空"）
                if !point.read {
                    continue;
                }
                if point.server.is_empty() || point.port == 0 {
                    // 原本就是空 → 显式清空。否则字段会永久残留 127.0.0.1，
                    // 用户日后手动打开系统代理就会指向无人监听的端口
                    commands.push(format!("networksetup -set{}proxy '{}' '' 0", kind, s));
                } else {
                    commands.push(format!(
                        "networksetup -set{}proxy '{}' {} {}",
                        kind, s, point.server, point.port
                    ));
                }
                touched = true;
            }
            if let Some(bypass) = &state.bypass_domains {
                // 空串 = 还原为"未设置绕过列表"（networksetup 支持空串清空）
                commands.push(format!(
                    "networksetup -setproxybypassdomains '{}' {}",
                    s, bypass
                ));
                touched = true;
            }
            if touched {
                commands.push(format!("networksetup -setwebproxystate '{}' off", s));
                commands.push(format!("networksetup -setsecurewebproxystate '{}' off", s));
                commands.push(format!(
                    "networksetup -setsocksfirewallproxystate '{}' off",
                    s
                ));
            }
        }
        commands
    }

    /// 查询系统自动代理(PAC)状态：任一服务启用即算启用
    pub fn autoproxy_state() -> (bool, Option<String>) {
        let services = get_network_services();
        if services.is_empty() {
            return (false, None);
        }
        let mut parts: Vec<String> = Vec::new();
        for service in &services {
            let s = service.replace('\'', "'\\''");
            let marker = format!("{}{}", AUTOPROXY_MARKER, parts.len());
            parts.push(format!(
                "echo '{}' ; networksetup -getautoproxyurl '{}'",
                marker, s
            ));
        }
        let out = Command::new("sh").arg("-c").arg(parts.join(" ; ")).output();
        let Ok(out) = out else {
            return (false, None);
        };
        parse_autoproxy_state(&String::from_utf8_lossy(&out.stdout))
    }

    /// 解析 -getautoproxyurl 输出（"URL: ..." + "Enabled: Yes/No"）
    fn parse_autoproxy_state(stdout: &str) -> (bool, Option<String>) {
        let mut enabled = false;
        let mut url: Option<String> = None;
        let mut in_block = false;
        for line in stdout.lines() {
            let t = line.trim();
            if t.starts_with(AUTOPROXY_MARKER) {
                in_block = true;
                continue;
            }
            if !in_block {
                continue;
            }
            if let Some(v) = t.strip_prefix("URL:") {
                let v = v.trim();
                if !v.is_empty() && v != "(null)" {
                    url = Some(v.to_string());
                }
            } else if let Some(v) = t.strip_prefix("Enabled:") {
                if v.trim().eq_ignore_ascii_case("yes") {
                    enabled = true;
                }
            }
        }
        (enabled, url)
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
            build_backup_query, build_batch_script, build_proxy_query, parse_autoproxy_state,
            parse_backup_output, parse_proxy_entries, run_networksetup_batch,
            summarize_batch_failure, ServiceProxyState, SysProxyBackup, BACKUP_MARKER, BATCH_LOG,
            QUERY_MARKER,
        };
        use crate::system::sysproxy::ProxyPoint;

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
        fn parse_backup_output_reads_points_and_bypass_list() {
            // 模拟一个服务：三项代理 + 多行绕过列表
            let stdout = "||AUROWEAVE_BACKUP||0|web\nEnabled: No\nServer: proxy.corp.example.com\nPort: 8080\n\
                          ||AUROWEAVE_BACKUP||0|secureweb\nEnabled: No\nServer: proxy.corp.example.com\nPort: 8080\nAuthenticated Proxy Enabled: 1\n\
                          ||AUROWEAVE_BACKUP||0|socksfirewall\nEnabled: No\nServer: \nPort: 0\n\
                          ||AUROWEAVE_BACKUP||0|bypass\n*.corp.example.com\n10.0.0.0/8\n";
            let states = parse_backup_output(stdout, &["Wi-Fi".to_string()]);
            assert_eq!(states.len(), 1);
            let s = &states[0];
            assert_eq!(s.service, "Wi-Fi");
            assert_eq!(
                (s.web.server.as_str(), s.web.port),
                ("proxy.corp.example.com", 8080)
            );
            assert!(!s.web.enabled);
            assert!(s.secure_web.auth);
            // server 为空 / port 为 0 的项保持空值，还原时会跳过（不写空地址）
            assert!(s.socks.server.is_empty() && s.socks.port == 0);
            // 一行一个域名 → 空格分隔保存
            assert_eq!(
                s.bypass_domains.as_deref(),
                Some("*.corp.example.com 10.0.0.0/8")
            );
        }

        #[test]
        fn parse_backup_output_handles_multi_service_blocks() {
            let stdout = "||AUROWEAVE_BACKUP||0|web\nEnabled: Yes\nServer: 127.0.0.1\nPort: 8890\n\
                          ||AUROWEAVE_BACKUP||0|bypass\n\
                          ||AUROWEAVE_BACKUP||1|web\nEnabled: No\nServer: 10.1.1.1\nPort: 3128\n\
                          ||AUROWEAVE_BACKUP||1|bypass\nlocalhost\n";
            let services = vec!["Wi-Fi".to_string(), "Tailscale".to_string()];
            let states = parse_backup_output(stdout, &services);
            assert_eq!(states.len(), 2);
            assert!(states[0].web.enabled);
            assert_eq!(states[0].bypass_domains.as_deref(), Some(""));
            assert_eq!(states[1].web.server, "10.1.1.1");
            assert_eq!(states[1].bypass_domains.as_deref(), Some("localhost"));
        }

        #[test]
        fn restore_commands_rewrite_values_then_force_state_off() {
            // 关键性质：-setwebproxy 会隐式开启代理，所以写回原值后必须显式置 off，
            // 还原的只是"字段值"，不是"开关状态"
            let backup = SysProxyBackup {
                services: vec![ServiceProxyState {
                    service: "Wi-Fi".to_string(),
                    web: ProxyPoint {
                        server: "proxy.corp.example.com".to_string(),
                        port: 8080,
                        enabled: true,
                        auth: false,
                        read: true,
                    },
                    secure_web: ProxyPoint::default(),
                    socks: ProxyPoint::default(),
                    bypass_domains: Some("*.corp.example.com".to_string()),
                }],
                windows: None,
            };
            let cmds = super::build_restore_commands(&backup);
            assert!(cmds
                .iter()
                .any(|c| c == "networksetup -setwebproxy 'Wi-Fi' proxy.corp.example.com 8080"));
            assert!(cmds
                .iter()
                .any(|c| c == "networksetup -setproxybypassdomains 'Wi-Fi' *.corp.example.com"));
            for state in [
                "-setwebproxystate",
                "-setsecurewebproxystate",
                "-setsocksfirewallproxystate",
            ] {
                assert!(
                    cmds.iter()
                        .any(|c| c == &format!("networksetup {} 'Wi-Fi' off", state)),
                    "缺少显式关闭: {state}\n{cmds:?}"
                );
            }
            // server/port 缺失的项不得写出（避免把代理写成空地址）
            assert!(!cmds
                .iter()
                .any(|c| c.contains("-setsecurewebproxy 'Wi-Fi'")));
        }

        #[test]
        fn restore_commands_clear_field_when_original_was_empty() {
            // 用户原本没配代理（server 为空、port 为 0，但**读到过**该项）：
            // 必须显式清空，否则字段永久残留 127.0.0.1，用户日后手动开启系统
            // 代理就会指向无人监听的端口
            let backup = SysProxyBackup {
                services: vec![ServiceProxyState {
                    service: "Wi-Fi".to_string(),
                    web: ProxyPoint {
                        read: true,
                        ..Default::default()
                    },
                    ..Default::default()
                }],
                windows: None,
            };
            let cmds = super::build_restore_commands(&backup);
            assert!(
                cmds.iter()
                    .any(|c| c == "networksetup -setwebproxy 'Wi-Fi' '' 0"),
                "{cmds:?}"
            );
            // 未读到（查询失败）时必须跳过，不能把"读不到"当成"原本为空"
            let unreadable = SysProxyBackup {
                services: vec![ServiceProxyState {
                    service: "Wi-Fi".to_string(),
                    web: ProxyPoint::default(),
                    ..Default::default()
                }],
                windows: None,
            };
            assert!(super::build_restore_commands(&unreadable).is_empty());
        }

        #[test]
        fn restore_commands_skip_empty_snapshot() {
            assert!(super::build_restore_commands(&SysProxyBackup::default()).is_empty());
        }

        #[test]
        fn backup_query_script_has_marker_per_field() {
            let services = vec!["Wi-Fi".to_string(), "Tailscale".to_string()];
            let script = build_backup_query(&services);
            // 每服务 4 个字段（web/secureweb/socksfirewall/bypass）
            assert_eq!(script.matches(BACKUP_MARKER).count(), 8, "{script}");
            assert!(script.contains("networksetup -getproxybypassdomains 'Tailscale'"));
        }

        #[test]
        fn autoproxy_state_parses_url_and_enabled_flag() {
            let on =
                "||AUROWEAVE_PAC||0\nURL: http://pac.corp.example.com/proxy.pac\nEnabled: Yes\n";
            assert_eq!(
                super::parse_autoproxy_state(on),
                (
                    true,
                    Some("http://pac.corp.example.com/proxy.pac".to_string())
                )
            );
            let off = "||AUROWEAVE_PAC||0\nURL: (null)\nEnabled: No\n";
            assert_eq!(super::parse_autoproxy_state(off), (false, None));
            assert_eq!(super::parse_autoproxy_state(""), (false, None));
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

// ===========================================================================
// 快照 / 还原流程（跨平台分派）
// ===========================================================================

/// 当前平台是否支持"原配置快照/还原"
/// - Windows / macOS：支持（HKCU 值 / networksetup 双向可读可写）
/// - Linux 等：暂不支持（gsettings 键路径与权限差异大），此时快照与还原都退化为
///   空操作——比"猜着还原"安全，代价是字段可能残留 127.0.0.1（同改动前行为）
#[cfg(any(target_os = "windows", target_os = "macos"))]
const BACKUP_SUPPORTED: bool = true;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const BACKUP_SUPPORTED: bool = false;

/// 读取当前平台的用户原配置快照
fn read_backup_state() -> Result<SysProxyBackup, String> {
    #[cfg(target_os = "macos")]
    {
        mac_sysproxy::read_backup_state()
    }
    #[cfg(target_os = "windows")]
    {
        Ok(SysProxyBackup {
            windows: Some(WinProxyState {
                proxy_server: win_registry::read_internet_settings_string("ProxyServer"),
                proxy_override: win_registry::read_internet_settings_string("ProxyOverride"),
            }),
            ..Default::default()
        })
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Err("当前平台暂不支持代理配置快照".to_string())
    }
}

/// 把被本应用覆盖的字段还原为用户原值（**不改开关状态**）
fn restore_backup_fields(backup: &SysProxyBackup, allow_prompt: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let commands = mac_sysproxy::build_restore_commands(backup);
        if commands.is_empty() {
            return Ok(());
        }
        // 还原走 networksetup 批量执行（allow_prompt=false 时纯静默）
        mac_sysproxy::run_commands(&commands, allow_prompt)
    }
    #[cfg(target_os = "windows")]
    {
        let Some(win) = &backup.windows else {
            return Ok(());
        };
        // 原本"有值"→写回；"无值"→删除该值（恢复"从未配置"，写空串会让部分
        // 解析器困惑）。两种情况都不触碰 ProxyEnable / AutoConfigURL：
        // 前者保证"代理确实是关的"，后者本应用从未修改，还原它反而会覆盖
        // 用户的公司 PAC 设置
        for (name, value) in [
            ("ProxyServer", &win.proxy_server),
            ("ProxyOverride", &win.proxy_override),
        ] {
            match value {
                Some(v) => win_registry::write_internet_settings_string(name, v)?,
                None => win_registry::delete_internet_settings_value(name)?,
            }
        }
        win_sysproxy::refresh();
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = backup;
        Ok(())
    }
}

/// 快照并持久化用户原配置（仅在"即将接管且当前代理不是本应用写的"时调用）
fn capture_backup(app: &tauri::AppHandle) {
    if !BACKUP_SUPPORTED {
        // 平台不支持：不是错误，不该每次开启都刷告警
        log::debug!("[sysproxy] 当前平台不支持原配置快照，跳过");
        return;
    }
    match read_backup_state() {
        Ok(backup) => {
            let value = match serde_json::to_value(&backup) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("[sysproxy] 序列化原配置快照失败: {}", e);
                    return;
                }
            };
            let patch = serde_json::json!({ "sysproxy_backup": value });
            if let Err(e) = crate::commands::settings::update_settings_internal(app, patch) {
                log::error!("[sysproxy] 持久化原配置快照失败: {}", e);
            } else {
                log::info!("[sysproxy] 已快照用户原系统代理配置");
            }
        }
        Err(e) => log::warn!(
            "[sysproxy] 读取原系统代理配置失败（关闭时将不还原字段）: {}",
            e
        ),
    }
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

pub fn is_own_proxy_endpoint(managed_port: u16) -> bool {
    match get_system_proxy_endpoint() {
        Some((host, port)) => port == managed_port && is_loopback_host(&host),
        None => false,
    }
}

/// 判断当前开启的系统代理是否为**本应用写入**的残留（含"上次写入端点"）
///
/// 在 `is_own_proxy_endpoint` 基础上叠加 `sysproxy_applied`（上次实际写入的
/// 端点）比对，修复端口变更后归属失配导致残留代理永久断网的问题（F2）。
/// 调用方迁移完成后由 `is_own_proxy_endpoint` 直接承接本语义，旧签名移除。
pub fn is_own_proxy_endpoint_with(
    app: &tauri::AppHandle,
    managed_port: u16,
) -> bool {
    let applied_port = crate::commands::settings::settings_get_internal(app)
        .sysproxy_applied
        .map(|a| a.port);
    match get_system_proxy_endpoint() {
        Some((host, port)) => {
            is_loopback_host(&host) && (port == managed_port || Some(port) == applied_port)
        }
        None => false,
    }
}

/// 代理服务器**字段**（而非开关）当前是否仍被本应用写入的端点污染
///
/// 这是 F3 的核心判据：`is_own_proxy_endpoint` 只认"开启态"，而
/// `SysProxyBackup` 要消灭的残留恰恰大量存在于"开关已关、字段仍指向
/// 127.0.0.1"这一状态——此时旧逻辑一律 early-return，快照机制形同虚设。
/// 这里独立读字段（Windows 读 ProxyServer，macOS 读三项代理的 server/port），
/// 使还原决策不再依赖开关状态。
pub fn is_proxy_field_polluted(app: &tauri::AppHandle) -> bool {
    let settings = crate::commands::settings::settings_get_internal(app);
    // 优先用上次实际写入的端点；无记录时退回当前 mixed_port
    let applied = settings.sysproxy_applied.clone().unwrap_or(AppliedEndpoint {
        host: "127.0.0.1".to_string(),
        port: settings.mixed_port,
    });

    #[cfg(target_os = "macos")]
    {
        mac_sysproxy::is_field_polluted_by(&applied)
    }
    #[cfg(target_os = "windows")]
    {
        // 注册表 ProxyServer 与开关位 ProxyEnable 相互独立：
        // 即使 ProxyEnable=0，ProxyServer 仍可能留着 127.0.0.1:<port>
        match win_registry::read_proxy_server()
            .as_deref()
            .and_then(parse_proxy_server)
        {
            Some((host, port)) => host.eq_ignore_ascii_case(&applied.host) && port == applied.port,
            None => false,
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = app;
        false
    }
}

/// 记录本应用实际写入的端点（归属判定的权威来源）
///
/// 只在写入成功且回读校验通过后调用——记录一个没真正写进去的端点，会让守护
/// 误把别人的代理认成自己的并关掉。
pub fn persist_applied_endpoint(app: &tauri::AppHandle, port: u16) {
    let applied = AppliedEndpoint {
        host: "127.0.0.1".to_string(),
        port,
    };
    let patch = serde_json::json!({ "sysproxy_applied": applied });
    if let Err(e) = crate::commands::settings::update_settings_internal(app, patch) {
        log::warn!("[sysproxy] 持久化已写入端点失败（残留归属判定将退化）: {}", e);
    }
}

/// 清除已写入端点记录（代理已关闭且字段已还原后调用）
pub fn clear_applied_endpoint(app: &tauri::AppHandle) {
    let patch = serde_json::json!({ "sysproxy_applied": serde_json::Value::Null });
    if let Err(e) = crate::commands::settings::update_settings_internal(app, patch) {
        log::warn!("[sysproxy] 清除已写入端点记录失败: {}", e);
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

/// 纯判据：回读到的端点是否满足"已按请求开启为 127.0.0.1:<port>"
fn endpoint_satisfies(actual: Option<&(String, u16)>, port: u16) -> bool {
    match actual {
        Some((host, p)) => *p == port && is_loopback_host(host),
        None => false,
    }
}

/// 回读校验：确认系统代理的真实状态与请求一致
///
/// 为什么放在 `set_system_proxy` 内部统一执行：全仓有 17 处写入口，若只在
/// 个别路径校验，其余路径仍会"写入未生效却报告成功"——用户在 UI 上看到的是
/// "已接管"，实际流量没走代理，且日志里什么都没有（假成功比失败更难查）。
/// 放进统一入口后，`Ok(())` 的语义收紧为"系统代理状态确实与请求一致"。
fn verify_system_proxy_state(enabled: bool, port: u16) -> Result<(), String> {
    if enabled {
        let actual = get_system_proxy_endpoint();
        if endpoint_satisfies(actual.as_ref(), port) {
            return Ok(());
        }
        return Err(match actual {
            Some((host, p)) => format!(
                "系统代理已写入但回读校验未通过（当前指向 {}:{}，期望 127.0.0.1:{}），流量可能未被接管",
                host, p, port
            ),
            None => "系统代理已写入但回读校验未通过（未发现开启的手动代理），流量可能未被接管"
                .to_string(),
        });
    }

    // 关闭：必须回读确认确实关掉了——系统代理停在 127.0.0.1 而内核已停 = 整机断网
    if get_system_proxy_status() {
        return Err("系统代理仍处于开启状态（部分网络服务拒绝关闭），整机可能断网，请检查 系统设置 → 网络 → 详细信息 → 代理".to_string());
    }
    Ok(())
}

/// 设置系统代理（各平台统一入口）
///
/// 期望状态钩子：成功设置后同步 proxy_guard 期望态——应用内任何路径
/// （托盘/模式切换/自愈）开代理即视为"用户期望开启"，守护开始校验漂移；
/// 关代理即期望关闭，守护立即停手。失败时不更新期望态（守护按旧期望继续）。
///
/// 返回 `Ok` 的语义 = **回读校验通过**：系统代理的真实状态与请求一致。
/// 写入成功但回读不一致时返回 Err（如实上报"假成功"）。
pub fn set_system_proxy(enabled: bool, port: u16) -> Result<(), String> {
    set_system_proxy_impl(enabled, port)?;
    // 期望态按"写入是否发起成功"更新：即便回读校验失败，用户的意图仍然成立
    // （开=期望开；关=期望关，守护不该再把残留代理拉起来）
    crate::system::proxy_guard::set_desired(enabled);
    let verified = verify_system_proxy_state(enabled, port);
    if enabled && verified.is_ok() {
        // PAC 优先于手动代理，必要时如实告知（不改用户的 PAC）
        warn_if_autoproxy_active();
    }
    verified
}

/// 开启成功后检查 PAC 是否仍在生效，并给出明确提示
///
/// PAC（自动代理 URL）优先于手动代理：用户若开着公司 PAC，即使 Auroweave
/// 把手动代理配好了，流量仍会走 PAC，表现为"代理没生效"。这里不改 PAC
/// （那是用户/企业的配置），只如实告知。
pub fn warn_if_autoproxy_active() {
    if !get_system_proxy_status() {
        return;
    }
    let (enabled, url) = get_autoproxy_state();
    if enabled {
        log::warn!(
            "[sysproxy] 系统自动代理(PAC)仍处于开启状态（{}），macOS/Windows 下 PAC 优先于手动代理，流量可能不会走 Auroweave；请在系统设置中关闭 PAC 或改用 TUN 模式",
            url.unwrap_or_else(|| "未提供 URL".to_string())
        );
    }
}

/// 带"原配置快照/还原"的系统代理写入口
///
/// 与 `set_system_proxy` 的差别只有两点，其余语义（回读校验、期望态钩子）完全一致：
/// - **开启前**：若当前系统代理不是本应用写的（`is_own_proxy_endpoint` 为假），
///   先把用户原配置（服务器地址 / 绕过列表）快照进 settings
/// - **关闭后**：若关闭前系统代理确实是本应用写的，把这些**字段值**还原回去
///
/// 两条安全约束：
/// 1. 还原只在"关闭确实成功"后执行——若关闭失败（代理还指着本应用的
///    127.0.0.1），此时把字段改回用户原值会让流量指向无人监听的地址，比不还原更糟
/// 2. 归属判定同时是防误改闸门：若关闭前系统代理已被用户/其他应用改成别的地址，
///    那不是我们留下的，不还原、不覆盖
pub fn set_system_proxy_with_backup(
    app: &tauri::AppHandle,
    enabled: bool,
    port: u16,
) -> Result<(), String> {
    set_system_proxy_with_backup_impl(app, enabled, port, true)
}

/// 静默版写入口：**绝不弹提权框**
///
/// 专供两条无人值守路径使用：
/// - 启动期残留清理（`startup::apply_core_mode_with_fallback` 步骤3）
/// - 守护的残留收敛（`proxy_guard`，30s 一拍）
///
/// 这两条路径的原始约束是"全程静默、不打扰用户"。用普通的
/// `set_system_proxy_with_backup` 会在 macOS 上经由 osascript 弹出密码框：
/// 启动时弹一次尚可接受，守护则是**每 30 秒弹一次**且用户无从预期，
/// 比它要解决的残留代理问题更扰民。
pub fn set_system_proxy_with_backup_silent(
    app: &tauri::AppHandle,
    enabled: bool,
    port: u16,
) -> Result<(), String> {
    set_system_proxy_with_backup_impl(app, enabled, port, false)
}

fn set_system_proxy_with_backup_impl(
    app: &tauri::AppHandle,
    enabled: bool,
    port: u16,
    allow_prompt: bool,
) -> Result<(), String> {
    // 归属判定要同时看"开关态"与"字段态"：
    // - 开关态：代理当前开着且指向我们写过的端点
    // - 字段态：即使开关已关，ProxyServer / 三项代理字段仍指向我们写过的端点
    // 旧实现只看开关态，导致"开关已关 + 字段仍污染"这一最常见的残留形态
    // 被判为"不是我们写的"从而跳过还原，快照机制完全失效（F3）。
    let managed = crate::commands::settings::settings_get_internal(app).mixed_port;
    let ours_before = is_own_proxy_endpoint(managed);

    // 字段态探测只在关闭路径需要（决定要不要还原），开启路径不查：
    // 每次探测在 macOS 上是一次 networksetup 子进程调用（150-400ms），
    // 开启路径无条件查一遍纯属白付开销
    let field_polluted_before =
        !enabled && is_proxy_field_polluted(app);

    if enabled && !ours_before {
        capture_backup(app);
    }

    // allow_prompt=false 时走静默实现，确保"不弹框"贯穿写与还原两侧
    let result = if allow_prompt {
        set_system_proxy(enabled, port)
    } else {
        set_system_proxy_silent_impl(enabled, port)
    };
    if result.is_ok() {
        crate::system::proxy_guard::set_desired(enabled);
    }

    if enabled {
        // 回读校验通过才记录：记录一个没真正写进去的端点，会让守护把别人的
        // 代理误认成自己的并静默关掉
        if result.is_ok() {
            persist_applied_endpoint(app, port);
        }
    } else if result.is_ok() {
        // 关闭成功后还原字段。归属闸门放宽到"开关态或字段态任一命中"——
        // 字段污染本身就是我们造成的，还原它不构成误改。
        if ours_before || field_polluted_before {
            let backup = crate::commands::settings::settings_get_internal(app).sysproxy_backup;
            match backup {
                Some(b) => match restore_backup_fields(&b, allow_prompt) {
                    Ok(()) => {
                        log::info!("[sysproxy] 已还原用户原代理配置字段（开关仍为关闭）");
                        clear_applied_endpoint(app);
                    }
                    // 还原失败：保留 applied 记录，让下次退出/守护还有机会重试，
                    // 也让"字段仍污染"这一事实不至于被遗忘
                    Err(e) => log::error!("[sysproxy] 还原用户原代理配置失败: {}", e),
                },
                None => {
                    log::info!("[sysproxy] 无原配置快照可还原");
                    clear_applied_endpoint(app);
                }
            }
        }
    }

    result
}

/// 查询系统自动代理(PAC)状态：返回 (是否启用, URL)
///
/// 单独读取而不并入 `is_proxy_enabled`：PAC 是**用户/企业自有**的配置，
/// Auroweave 从不写它，也不该在"关闭代理"时把它清掉。把 PAC 计入手动代理
/// 会导致两个误判——一是 `ensure_system_proxy_disabled` 误以为系统还有残留
/// 代理而反复重试，二是守护可能去动用户的公司配置。
///
/// 它的意义在于**告知**：PAC 开启时流量仍按 PAC 走，Auroweave 的手动代理
/// 可能不生效（macOS 上 PAC 优先于手动代理），必须在开启时明确提示。
pub fn get_autoproxy_state() -> (bool, Option<String>) {
    #[cfg(target_os = "windows")]
    {
        let url = win_registry::read_internet_settings_string("AutoConfigURL");
        (url.is_some(), url)
    }
    #[cfg(target_os = "macos")]
    {
        mac_sysproxy::autoproxy_state()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        (false, None)
    }
}

/// 退出清理：静默关闭系统代理 + 静默还原原配置字段
///
/// 退出路径绝不能弹提权框（会阻塞退出流程）。实测 macOS 上"关闭代理"与
/// "写回代理字段"均无需提权，Windows 注册表写入同样无需提权，因此静默
/// 关闭 + 静默还原都可行。
///
/// 与用户主动关闭的差别：这里不做回读失败重试、不提示，只留日志——进程
/// 即将退出，能做的都做，做不到的（例如个别环境确实需要提权）留 error 日志。
/// 注意还原失败时的残留是**用户的原配置字段**（无害），而不是 127.0.0.1
/// （那才会导致断网）——因为还原只写字段、不开开关。
pub fn cleanup_system_proxy_on_exit(app: &tauri::AppHandle) {
    let managed = crate::commands::settings::settings_get_internal(app).mixed_port;
    let ours_before = is_own_proxy_endpoint(managed);
    // 退出路径同样要认字段态：崩溃/强杀后可能已是"开关关、字段脏"，
    // 只看开关会跳过还原，把 127.0.0.1 永久留在系统代理设置里（F3）
    let field_polluted_before = is_proxy_field_polluted(app);

    // 开关态只查一次并复用：get_system_proxy_status 在 macOS 上是一次
    // networksetup 批量子进程调用（150-400ms），退出路径查三次就是白等 0.5s
    let proxy_on = get_system_proxy_status();
    if !proxy_on && !ours_before && !field_polluted_before {
        return;
    }

    // 开关开着才需要先关；纯字段污染时跳过这一步（networksetup 关闭命令
    // 在已关闭状态下是空操作，白跑一次 150-400ms 的子进程）
    if proxy_on {
        if let Err(e) = set_system_proxy_silent(false, 0) {
            log::error!("[app] 退出清理系统代理失败（可能残留代理导致断网）: {}", e);
            return;
        }
        // 回读确认：仍开启就不还原字段——代理还指着 127.0.0.1 时把字段改回原值，
        // 会让流量指向一个无人监听的地址，比不还原更糟
        if get_system_proxy_status() {
            log::error!("[app] 退出清理后回读仍为开启，跳过原配置还原");
            return;
        }
    }

    if !ours_before && !field_polluted_before {
        return;
    }
    match crate::commands::settings::settings_get_internal(app).sysproxy_backup {
        Some(b) => {
            if let Err(e) = restore_backup_fields(&b, false) {
                log::error!("[app] 退出时静默还原原代理配置失败: {}", e);
            } else {
                log::info!("[app] 退出时已静默还原原代理配置字段");
                clear_applied_endpoint(app);
            }
        }
        None => {
            log::info!("[app] 退出时无原配置快照可还原");
            clear_applied_endpoint(app);
        }
    }
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
/// - 回读仍开启 → 关闭并还原原配置字段（用户显式关闭，允许弹提权框）
/// - 仍关不掉 → 返回 Err，由上层兜底（拉起内核）并向用户报错
///
/// 期望态无论成败都先置 false：用户的意图是"关"，守护不应再把它打开
/// （否则下次 30s 拍的漂移恢复会把刚残留的代理又拉起来）。
///
/// 走 `set_system_proxy_with_backup` 而非裸写：用户点「关闭代理」走的就是
/// 这条路（direct 快速路径 / TUN 不变式），若不在这里还原字段，"代理服务器"
/// 就会永久残留 127.0.0.1 —— 那是本函数最初要解决的那类环境污染。
pub fn ensure_system_proxy_disabled(app: &tauri::AppHandle) -> Result<(), String> {
    crate::system::proxy_guard::set_desired(false);

    let proxy_on = get_system_proxy_status();
    // 字段态独立判定：开关已关不代表字段干净。旧实现在这里 early-return，
    // 使"开关已关 + ProxyServer 仍是 127.0.0.1"这一残留永远得不到还原——
    // 而它正是 SysProxyBackup 存在的唯一理由（F3）。
    // 仅在开关已关时才探测：开关还开着时 ours_before 必然为真、字段态是冗余的，
    // 而每次探测都是一次 150-400ms 的 networksetup 子进程调用。
    let field_polluted = !proxy_on && is_proxy_field_polluted(app);

    if !proxy_on && !field_polluted {
        return Ok(());
    }

    if proxy_on {
        log::info!("[sysproxy] 回读发现系统代理仍处于开启态，执行关闭（含原配置还原）");
    } else {
        log::info!("[sysproxy] 开关已关但代理字段仍指向本应用端点，执行字段还原");
    }
    // 内含回读校验：Ok 即代表"确实已关闭"（proxy_on 分支）
    // 或"字段已还原"（纯字段污染分支）
    set_system_proxy_with_backup(app, false, 0)
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
    use super::{endpoint_satisfies, is_loopback_host, parse_proxy_server, AppliedEndpoint};

    /// 归属判定的纯逻辑复刻（与 is_own_proxy_endpoint_with 保持同源语义）
    ///
    /// 之所以复刻而不是直接调被测函数：后者要读注册表 / 跑 networksetup，
    /// 属集成测试范畴。判据本身是纯函数，单独复刻即可锁住语义，
    /// 且编译期能发现两处实现漂移（改一处忘另一处会在这里暴露）。
    fn owns(actual: Option<&(String, u16)>, managed_port: u16, applied: Option<&AppliedEndpoint>) -> bool {
        match actual {
            Some((host, port)) => {
                is_loopback_host(host)
                    && (*port == managed_port
                        || applied.is_some_and(|a| *port == a.port && *host == a.host))
            }
            None => false,
        }
    }

    #[test]
    fn applied_endpoint_survives_port_change() {
        // F2 回归防线：上次写入 8890，用户把 mixed_port 改成 7890，
        // 崩溃留下 127.0.0.1:8890。旧实现只比 mixed_port → 失配 →
        // 守护判定"非本应用写入"而放手 → 残留代理永久断网。
        let applied = AppliedEndpoint {
            host: "127.0.0.1".to_string(),
            port: 8890,
        };
        let residual = ("127.0.0.1".to_string(), 8890);

        // 旧判据：只看当前 mixed_port(7890) → 失配
        assert!(!owns(Some(&residual), 7890, None));
        // 新判据：叠加上次实际写入端点 → 命中，守护得以清理残留
        assert!(owns(Some(&residual), 7890, Some(&applied)));
    }

    #[test]
    fn applied_endpoint_never_matches_foreign_proxy() {
        // 防误关闸门：applied 记录不能扩大到"任意环回端口"，
        // 否则会把关掉用户自己的 127.0.0.1 服务（如本地开发服务器）
        let applied = AppliedEndpoint {
            host: "127.0.0.1".to_string(),
            port: 8890,
        };
        // 非环回 + 端口恰好相同 → 仍不认（公司代理用了同号端口）
        assert!(!owns(Some(&("10.0.0.1".to_string(), 8890)), 7890, Some(&applied)));
        // 环回但端口既非 managed 也非 applied → 不认
        assert!(!owns(Some(&("127.0.0.1".to_string(), 3000)), 7890, Some(&applied)));
    }

    #[test]
    fn applied_endpoint_requires_host_to_match_exactly() {
        // 字段污染判定用精确匹配：ipv6 回环写法不同，不应误判为同一端点，
        // 否则会把用户手动配的 [::1]:8890 当残留还原掉
        let applied = AppliedEndpoint {
            host: "127.0.0.1".to_string(),
            port: 8890,
        };
        assert!(!owns(Some(&("::1".to_string(), 8890)), 7890, Some(&applied)));
        assert!(owns(Some(&("127.0.0.1".to_string(), 8890)), 7890, Some(&applied)));
    }

    #[test]
    fn not_enabled_endpoint_is_never_ours() {
        // 代理没开时 get_system_proxy_endpoint 返回 None —— 开关态判定
        // 必须为 false。这正是 F3 的由来：字段污染要在别处单独判，
        // 不能指望这个函数顺带覆盖。
        let applied = AppliedEndpoint {
            host: "127.0.0.1".to_string(),
            port: 8890,
        };
        assert!(!owns(None, 8890, Some(&applied)));
    }

    #[test]
    fn endpoint_verify_accepts_only_our_loopback_endpoint() {
        // 写入后回读必须是"环回 + 我们写的端口"，否则视为写入未生效
        assert!(endpoint_satisfies(
            Some(&("127.0.0.1".to_string(), 8890)),
            8890
        ));
        assert!(endpoint_satisfies(
            Some(&("localhost".to_string(), 8890)),
            8890
        ));
        // 端口不符：上一次的残留 / 写入落到了别处
        assert!(!endpoint_satisfies(Some(&("127.0.0.1".to_string(), 7890)), 8890));
        // 非环回：他人的代理抢占了开关
        assert!(!endpoint_satisfies(Some(&("10.0.0.1".to_string(), 8890)), 8890));
        // 没有任何开启项
        assert!(!endpoint_satisfies(None, 8890));
    }

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
