/// Windows 系统服务控制封装 (SCM & UAC 提权执行)
/// 作者: TanXiang
///
/// 本模块封装了两类提权控制能力：
///
/// 1. **SCM 服务控制**（服务模式）：
///    - 通过 Windows Service Control Manager (SCM) API 直接查询/启动/停止系统服务
///    - 服务名: `AuroweaveCoreService`
///    - 由 AuroDaemon.exe 注册为 `SERVICE_WIN32_OWN_PROCESS`，以 SYSTEM 权限运行
///    - 主程序通过 `ipc_client` 具名管道向服务发送内核控制指令
///
/// 2. **计划任务控制**（本地模式 TUN 提权）：
///    - 通过 `schtasks` 命令触发预注册的提权计划任务 `AuroweaveDirectTunTask`
///    - 计划任务以最高权限运行 AuroDaemon.exe `run-task`，拉起 sing-box TUN 模式
///    - 避免每次启动 TUN 都弹出 UAC 窗口
///
/// 3. **UAC 提权执行**（安装/卸载）：
///    - 通过 `ShellExecuteExW` + `runas` 动词拉起提权子进程执行安装/卸载
///    - 等待子进程退出并检查退出码
#[cfg(target_os = "windows")]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SERVICE_DOES_NOT_EXIST, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceStatus, StartServiceW, SC_MANAGER_CONNECT, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_CONTROL_STOP, SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_STATUS, SERVICE_STOP
    };
    const SERVICE_RUNNING: u32 = 4;
    const SERVICE_STOPPED: u32 = 1;
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    // WAIT_TIMEOUT: WaitForSingleObject 超时返回值；UAC 提权子进程最多等待 120 秒
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, WAIT_TIMEOUT};

    /// UAC 提权子进程等待上限（毫秒）。
    /// 旧实现使用 INFINITE 等待：UAC 弹窗无人响应会永久挂起主程序，
    /// 120 秒足够用户在正常节奏下完成密码输入/取消。
    const UAC_WAIT_TIMEOUT_MS: u32 = 120_000;

    const SERVICE_NAME: &str = "AuroweaveCoreService";

    /// 查询系统服务当前状态
    ///
    /// 通过 SCM API 查询 `AuroweaveCoreService` 的运行状态。
    /// 返回值: "running" / "stopped" / "starting" (含 pending) / "not_installed"
    pub fn query_service_status() -> Result<String, String> {
        let service_name_w: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE);
            if scm == 0 {
                return Err("无法连接至 Windows SCM 服务".to_string());
            }

            let service = OpenServiceW(scm, service_name_w.as_ptr(), SERVICE_QUERY_STATUS);
            if service == 0 {
                let err = std::io::Error::last_os_error();
                CloseServiceHandle(scm);
                if err.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST as i32) {
                    return Ok("not_installed".to_string());
                }
                return Err(format!("查询服务失败: {}", err));
            }

            let mut status: SERVICE_STATUS = std::mem::zeroed();
            let res = QueryServiceStatus(service, &mut status);
            CloseServiceHandle(service);
            CloseServiceHandle(scm);

            if res == 0 {
                return Err("获取服务状态失败".to_string());
            }

            match status.dwCurrentState {
                SERVICE_RUNNING => Ok("running".to_string()),
                SERVICE_STOPPED => Ok("stopped".to_string()),
                _ => Ok("starting".to_string()), // 正在启动或停止归类为 starting/pending
            }
        }
    }

    /// 启动系统服务
    ///
    /// 通过 SCM API 调用 `StartServiceW` 启动 `AuroweaveCoreService`。
    /// 服务启动后 SCM 会拉起 AuroDaemon.exe `run` 子命令进入服务主体。
    pub fn start_service() -> Result<(), String> {
        let service_name_w: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
            if scm == 0 {
                return Err("连接 SCM 失败".to_string());
            }

            let service = OpenServiceW(scm, service_name_w.as_ptr(), SERVICE_START | SERVICE_QUERY_STATUS);
            if service == 0 {
                let err = std::io::Error::last_os_error();
                CloseServiceHandle(scm);
                return Err(format!("无法打开服务: {}", err));
            }

            let res = StartServiceW(service, 0, std::ptr::null());
            CloseServiceHandle(service);
            CloseServiceHandle(scm);

            if res == 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("服务启动失败: {}", err));
            }

            Ok(())
        }
    }

    /// 停止系统服务
    ///
    /// 通过 SCM API 调用 `ControlService` 发送 `SERVICE_CONTROL_STOP` 信号。
    /// 服务收到 Stop 信号后优雅关闭 IPC 服务并停止 sing-box 子进程。
    pub fn stop_service() -> Result<(), String> {
        let service_name_w: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
            if scm == 0 {
                return Err("连接 SCM 失败".to_string());
            }

            let service = OpenServiceW(scm, service_name_w.as_ptr(), SERVICE_STOP | SERVICE_QUERY_STATUS);
            if service == 0 {
                let err = std::io::Error::last_os_error();
                CloseServiceHandle(scm);
                return Err(format!("无法打开服务: {}", err));
            }

            let mut status: SERVICE_STATUS = std::mem::zeroed();
            let res = ControlService(service, SERVICE_CONTROL_STOP, &mut status);
            CloseServiceHandle(service);
            CloseServiceHandle(scm);

            if res == 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("服务停止失败: {}", err));
            }

            Ok(())
        }
    }

    /// 通过 UAC 提权执行服务安装/卸载操作
    ///
    /// 流程：
    /// 1. 定位 AuroDaemon.exe（先找资源目录，再找开发目录）
    /// 2. 使用 `ShellExecuteExW` + verb="runas" 弹出 UAC 提权窗口
    /// 3. 以 SW_HIDE 隐藏窗口模式运行，传递 action 作为命令行参数
    /// 4. 等待提权子进程退出并检查退出码（非 0 视为失败）
    pub fn execute_uac_action(app_handle: &tauri::AppHandle, action: &str) -> Result<(), String> {
        use tauri::Manager;
        
        // 步骤1: 定位 AuroDaemon.exe
        // 优先从 Tauri 资源目录查找，找不到再从当前可执行文件目录查找（开发环境）
        let mut svc_path = match app_handle.path().resource_dir() {
            Ok(dir) => dir.join("resources").join("AuroDaemon.exe"),
            Err(_) => std::path::PathBuf::new(),
        };

        if !svc_path.exists() {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let debug_path = exe_dir.join("AuroDaemon.exe");
                    if debug_path.exists() {
                        svc_path = debug_path;
                    }
                }
            }
        }

        if !svc_path.exists() {
            return Err("找不到服务宿主可执行文件，即使在开发目录中也未找到".to_string());
        }

        // 步骤2: 构建 ShellExecuteExW 参数
        // verb="runas" 触发 UAC 提权，nShow=0 (SW_HIDE) 隐藏窗口
        let verb_w: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
        let file_w: Vec<u16> = svc_path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        let params_w: Vec<u16> = action.encode_utf16().chain(std::iter::once(0)).collect();
        
        let dir_w: Vec<u16> = if let Some(parent) = svc_path.parent() {
            parent.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect()
        } else {
            vec![0]
        };

        // 步骤3: 调用 ShellExecuteExW 拉起提权进程并等待退出
        unsafe {
            let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_NOCLOSEPROCESS;
            info.hwnd = 0;
            info.lpVerb = verb_w.as_ptr();
            info.lpFile = file_w.as_ptr();
            info.lpParameters = params_w.as_ptr();
            info.lpDirectory = dir_w.as_ptr();
            info.nShow = 0; // SW_HIDE

            let res = ShellExecuteExW(&mut info);
            if res == 0 {
                let err = std::io::Error::last_os_error();
                return Err(format!("UAC 弹窗被拒绝或提权启动失败: {}", err));
            }

            // 等待提权子进程退出并检查退出码（120 秒超时，防 UAC 弹窗无人响应时永久挂起）
            if info.hProcess != 0 && info.hProcess != INVALID_HANDLE_VALUE {
                let wait_res = WaitForSingleObject(info.hProcess, UAC_WAIT_TIMEOUT_MS);
                if wait_res == WAIT_TIMEOUT {
                    CloseHandle(info.hProcess);
                    return Err("等待提权操作超时（120 秒）：用户取消或未响应 UAC 提示".to_string());
                }
                let mut exit_code: u32 = 0;
                let get_exit_res = GetExitCodeProcess(info.hProcess, &mut exit_code);
                CloseHandle(info.hProcess);

                if get_exit_res == 0 {
                    return Err("无法获取子进程退出状态".to_string());
                }

                if exit_code != 0 {
                    return Err(format!("服务操作子进程异常退出，退出码为: {}", exit_code));
                }
            } else {
                return Err("未成功获取提权进程句柄".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub fn query_service_status() -> Result<String, String> {
    win::query_service_status()
}

#[cfg(target_os = "windows")]
pub fn start_service() -> Result<(), String> {
    win::start_service()
}

#[cfg(target_os = "windows")]
pub fn stop_service() -> Result<(), String> {
    win::stop_service()
}

    /// 通过 UAC 提权安装系统服务或计划任务
    ///
    /// 根据 `run_mode` 决定安装方式：
    /// - `service`: 传递 `install-service --singbox <path>`，注册为 Windows 系统服务
    /// - 其他: 传递 `install-task --singbox <path>`，仅注册计划任务（本地模式 TUN 提权）
    #[cfg(target_os = "windows")]
    pub fn install_service_uac(app_handle: &tauri::AppHandle, run_mode: &str, singbox_path: &std::path::Path) -> Result<(), String> {
    let action = if run_mode == "service" {
        format!("install-service --singbox \"{}\"", singbox_path.to_string_lossy())
    } else {
        format!("install-task --singbox \"{}\"", singbox_path.to_string_lossy())
    };
    win::execute_uac_action(app_handle, &action)
}

    /// 通过 UAC 提权卸载系统服务与计划任务
    ///
    /// 传递 `uninstall` 参数给 AuroDaemon.exe，由提权进程执行完整卸载流程。
    #[cfg(target_os = "windows")]
    pub fn uninstall_service_uac(app_handle: &tauri::AppHandle) -> Result<(), String> {
    win::execute_uac_action(app_handle, "uninstall")
}

// ============ 计划任务控制方法 (Local 模式静默提权) ============

/// 触发预注册的提权计划任务以启动 TUN 模式 sing-box
///
/// 本地模式下 TUN 需要管理员权限创建虚拟网卡，通过预注册的计划任务
/// `AuroweaveDirectTunTask` 实现免 UAC 弹窗的静默提权启动。
///
/// 完整流程：
/// 1. **前置检查**：确认 `%ProgramData%\Auroweave\bin\AuroDaemon.exe` 存在
/// 2. **写入 PID**：将主进程 PID 写入 `parent.pid` 供看门狗轮询
/// 3. **生成 manifest**：计算 AuroDaemon.exe 和 sing-box.exe 的 SHA-256 哈希，
///    写入 `manifest.json` 供服务端自更新比对
/// 4. **触发任务**：执行 `schtasks /run /tn AuroweaveDirectTunTask`
#[cfg(target_os = "windows")]
pub fn run_direct_tun_task(app_handle: &tauri::AppHandle) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    // 步骤1: 前置检查，确认底层服务程序文件存在
    // 即使计划任务已注册，如果可执行文件被删除也需要提示重新安装
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let target_svc_path = std::path::PathBuf::from(&program_data).join("Auroweave").join("bin").join("AuroDaemon.exe");
    if !target_svc_path.exists() {
        return Err("底层服务程序文件缺失，请重新执行一键安装提权组件".to_string());
    }

    // 步骤2: 写入主进程 PID 供后台看门狗轮询退出
    // AuroDaemon 启动后会读取此文件并周期性检测主进程是否存活，若主进程退出则自动终止 sing-box
    let pid = std::process::id();
    let config_dir = crate::get_config_dir();
    let pid_file = config_dir.join("parent.pid");
    let _ = std::fs::write(pid_file, pid.to_string());
    
    // 步骤3: 生成 manifest.json 以供服务端自更新
    // 包含 AuroDaemon.exe 和 sing-box.exe 的路径与 SHA-256 哈希，服务启动时比对实现热更新
    let manifest_file = config_dir.join("manifest.json");
    use tauri::Manager;
    let mut svc_path = app_handle.path().resource_dir().unwrap_or_default().join("resources").join("AuroDaemon.exe");
    if !svc_path.exists() {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let debug_path = exe_dir.join("AuroDaemon.exe");
                if debug_path.exists() {
                    svc_path = debug_path;
                }
            }
        }
    }
    
    let singbox_path = crate::core::sidecar::SidecarManager::resolve_binary_path()
        .unwrap_or_else(|_| std::path::PathBuf::new());
        
    fn compute_sha256(path: &std::path::Path) -> String {
        use sha2::{Sha256, Digest};
        use std::io::Read;
        use std::sync::{OnceLock, Mutex};
        use std::collections::HashMap;
        use std::time::SystemTime;

        static HASH_CACHE: OnceLock<Mutex<HashMap<String, (SystemTime, u64, String)>>> = OnceLock::new();
        let cache_mutex = HASH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return String::new(),
        };
        let mtime = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let size = metadata.len();
        let path_str = path.to_string_lossy().to_string();

        if let Ok(cache) = cache_mutex.lock() {
            if let Some((cached_mtime, cached_size, cached_hash)) = cache.get(&path_str) {
                if *cached_mtime == mtime && *cached_size == size {
                    return cached_hash.clone();
                }
            }
        }

        if let Ok(mut file) = std::fs::File::open(path) {
            let mut hasher = Sha256::new();
            // 扩大缓冲区至 64KB，显著提升大文件的磁盘 I/O 读取速度
            let mut buffer = [0; 65536];
            while let Ok(n) = file.read(&mut buffer) {
                if n == 0 { break; }
                hasher.update(&buffer[..n]);
            }
            let hash = hasher.finalize();
            let hash_str = hash.iter().map(|b| format!("{:02x}", b)).collect::<String>();
            
            if let Ok(mut cache) = cache_mutex.lock() {
                cache.insert(path_str, (mtime, size, hash_str.clone()));
            }
            
            hash_str
        } else {
            String::new()
        }
    }
    
    // 计算 AuroDaemon.exe 和 sing-box.exe 的 SHA-256 哈希（带缓存，避免重复计算）
    let svc_hash = compute_sha256(&svc_path);
    let singbox_hash = compute_sha256(&singbox_path);
    
    let manifest = serde_json::json!({
        "svc_path": svc_path.to_string_lossy(),
        "svc_hash": svc_hash,
        // svc_version 供服务端自更新做版本单调递增校验（防降级攻击），
        // 缺失该字段时服务端会拒绝自替换
        "svc_version": env!("CARGO_PKG_VERSION"),
        "singbox_path": singbox_path.to_string_lossy(),
        "singbox_hash": singbox_hash
    });

    // manifest 属于消费方（提权进程/服务）的信任输入，写入失败必须上抛而非静默
    if let Err(e) = std::fs::write(&manifest_file, manifest.to_string()) {
        return Err(format!("写入 manifest.json 失败: {}", e));
    }

    // 步骤4: 执行 schtasks /run 触发预注册的提权计划任务
    log::info!("执行 schtasks /run /tn AuroweaveDirectTunTask");
    let mut cmd = std::process::Command::new("schtasks");
    cmd.arg("/run")
        .arg("/tn")
        .arg("AuroweaveDirectTunTask")
        .creation_flags(CREATE_NO_WINDOW);
        
    let status = cmd.status()
        .map_err(|e| format!("无法启动 schtasks 命令: {}", e))?;
    if !status.success() {
        return Err("启动提权计划任务失败，请确认是否已成功安装提权组件（可通过主界面一键安装）".to_string());
    }
    Ok(())
}

/// 停止提权计划任务并强杀残留进程
///
/// 流程：
/// 1. 执行 `schtasks /end` 终止计划任务
/// 2. 等待 300ms 让退出信号生效（异步 sleep，不阻塞运行时线程）
/// 3. 检测 AuroDaemon 和 sing-box 残留进程：
///    - 优先读取 PID 文件按 PID 精确终止（taskkill /F /PID）
///    - PID 不可得时才退化为按名称强杀（最后手段）
#[cfg(target_os = "windows")]
pub async fn stop_direct_tun_task() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    log::info!("执行 schtasks /end /tn AuroweaveDirectTunTask");
    let mut cmd = std::process::Command::new("schtasks");
    cmd.arg("/end")
        .arg("/tn")
        .arg("AuroweaveDirectTunTask")
        .creation_flags(CREATE_NO_WINDOW);

    let _ = cmd.status();

    // 等待计划任务退出信号生效（异步 sleep：本函数被 startup.rs 的 async 链路调用）
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // 使用共用的模块函数高效检测残留进程
    let running_status = crate::system::process::check_processes_running(&[
        crate::system::process::PROCESS_NAME_DAEMON,
        crate::system::process::PROCESS_NAME_SINGBOX,
    ]);

    let has_daemon = *running_status.get(crate::system::process::PROCESS_NAME_DAEMON).unwrap_or(&false);
    let has_singbox = *running_status.get(crate::system::process::PROCESS_NAME_SINGBOX).unwrap_or(&false);

    // 提权任务模式下 AuroDaemon 会将 sing-box 的 PID 写入 config_dir 下的 pid 文件
    let daemon_pid = read_core_pid_file("singbox.pid");
    let svc_pid = read_core_pid_file("svc.pid");

    if has_daemon {
        log::info!("AuroDaemon 残留，按 PID 精确终止...");
        let killed = match svc_pid {
            Some(pid) => crate::system::process::force_kill_by_pid(pid),
            None => {
                // PID 不可得：按名查 PID 再逐个终止，避免 taskkill /IM 全局误杀
                let pids = crate::system::process::find_pids_by_name(
                    &format!("{}.exe", crate::system::process::PROCESS_NAME_DAEMON),
                );
                if pids.is_empty() {
                    crate::system::process::force_kill_process(&format!(
                        "{}.exe",
                        crate::system::process::PROCESS_NAME_DAEMON
                    ))
                } else {
                    pids.iter().all(|pid| crate::system::process::force_kill_by_pid(*pid))
                }
            }
        };
        if !killed {
            log::warn!("AuroDaemon 残留进程终止失败（可能已自行退出）");
        }
    }

    if has_singbox {
        log::info!("sing-box 残留，按 PID 精确终止...");
        let killed = match daemon_pid {
            Some(pid) => crate::system::process::force_kill_by_pid(pid),
            None => {
                let pids = crate::system::process::find_pids_by_name(
                    &format!("{}.exe", crate::system::process::PROCESS_NAME_SINGBOX),
                );
                if pids.is_empty() {
                    crate::system::process::force_kill_process(&format!(
                        "{}.exe",
                        crate::system::process::PROCESS_NAME_SINGBOX
                    ))
                } else {
                    pids.iter().all(|pid| crate::system::process::force_kill_by_pid(*pid))
                }
            }
        };
        if !killed {
            log::warn!("sing-box 残留进程终止失败（可能已自行退出）");
        }
    }

    Ok(())
}

/// 读取 config_dir 下的内核/守护进程 PID 文件
///
/// 返回 None 表示文件缺失或内容非数字（此时调用方应退化为按进程名处理）。
#[cfg(target_os = "windows")]
fn read_core_pid_file(name: &str) -> Option<u32> {
    let pid_file = crate::get_config_dir().join(name);
    let content = std::fs::read_to_string(pid_file).ok()?;
    content.trim().parse::<u32>().ok()
}

/// 查询提权计划任务是否正在运行
///
/// 通过 `schtasks /query` 查询任务状态，解析 CSV 输出判断是否为 "Running"。
/// 同时支持英文和中文（GBK 编码）的系统语言输出。
#[cfg(target_os = "windows")]
pub fn query_direct_tun_task_running() -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let mut cmd = std::process::Command::new("schtasks");
    cmd.arg("/query")
        .arg("/tn")
        .arg("AuroweaveDirectTunTask")
        .arg("/fo")
        .arg("CSV")
        .arg("/nh")
        .creation_flags(CREATE_NO_WINDOW);
        
    let output = cmd.output()
        .map_err(|e| format!("查询计划任务状态失败: {}", e))?;
    if !output.status.success() {
        return Ok(false);
    }
    
    let bytes = &output.stdout;
    
    // 判断任务是否正在运行（schtasks /query 输出的状态列）
    // 注意: 只有 "Running"/"正在运行" 才表示任务正在执行
    // "Ready"/"准备就绪" 表示任务已就绪但未运行，"Unknown" 也表示未运行
    
    // ASCII "Running" (7 bytes)
    let is_running_en = bytes.windows(7).any(|w| w == b"Running");
    
    // GBK "正在运行" (8 bytes) -> 正=D5FD, 在=D4DA, 运=D4CB, 行=D0D0
    let gbk_running = [0xd5, 0xfd, 0xd4, 0xda, 0xd4, 0xcb, 0xd0, 0xd0];
    let is_running_zh = bytes.windows(8).any(|w| w == gbk_running);
    
    Ok(is_running_en || is_running_zh)
}

/// 检测 sing-box 进程是否正在运行
///
/// 委托 `system::process` 模块通过进程名检测，用于 TUN 模式启动后的轮询确认。
#[cfg(target_os = "windows")]
pub fn query_singbox_process_running() -> bool {
    crate::system::process::is_process_running(crate::system::process::PROCESS_NAME_SINGBOX)
}

// 非 Windows 平台空桩实现
#[cfg(not(target_os = "windows"))]
pub fn query_service_status() -> Result<String, String> {
    Ok("not_installed".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn start_service() -> Result<(), String> {
    Err("当前平台不支持系统服务模式".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn stop_service() -> Result<(), String> {
    Err("当前平台不支持系统服务模式".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn install_service_uac(_app_handle: &tauri::AppHandle, _run_mode: &str, _singbox_path: &std::path::Path) -> Result<(), String> {
    Err("当前平台不支持系统服务模式".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn uninstall_service_uac(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    Err("当前平台不支持系统服务模式".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn run_direct_tun_task(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    Err("当前平台不支持静默提权任务".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn stop_direct_tun_task() -> Result<(), String> {
    Err("当前平台不支持静默提权任务".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn query_direct_tun_task_running() -> Result<bool, String> {
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
pub fn query_singbox_process_running() -> bool {
    false
}
