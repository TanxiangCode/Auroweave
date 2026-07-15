/// Windows 系统服务控制封装 (SCM & UAC 提权执行)
/// 作者: TanXiang

#[cfg(target_os = "windows")]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_SERVICE_DOES_NOT_EXIST, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceStatus, StartServiceW, SC_MANAGER_CONNECT, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_CONTROL_STOP, SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_STATUS, SERVICE_STOP
    };
    const SERVICE_RUNNING: u32 = 4;
    const SERVICE_STOPPED: u32 = 1;
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};

    const SERVICE_NAME: &str = "AuroweaveCoreService";

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

    pub fn execute_uac_action(app_handle: &tauri::AppHandle, action: &str) -> Result<(), String> {
        use tauri::Manager;
        
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

        let verb_w: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
        let file_w: Vec<u16> = svc_path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        let params_w: Vec<u16> = action.encode_utf16().chain(std::iter::once(0)).collect();
        
        let dir_w: Vec<u16> = if let Some(parent) = svc_path.parent() {
            parent.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect()
        } else {
            vec![0]
        };

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

            if info.hProcess != 0 && info.hProcess != INVALID_HANDLE_VALUE {
                WaitForSingleObject(info.hProcess, INFINITE);
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

#[cfg(target_os = "windows")]
pub fn install_service_uac(app_handle: &tauri::AppHandle, run_mode: &str, singbox_path: &std::path::Path) -> Result<(), String> {
    let action = if run_mode == "service" {
        format!("install-service --singbox \"{}\"", singbox_path.to_string_lossy())
    } else {
        format!("install-task --singbox \"{}\"", singbox_path.to_string_lossy())
    };
    win::execute_uac_action(app_handle, &action)
}

#[cfg(target_os = "windows")]
pub fn uninstall_service_uac(app_handle: &tauri::AppHandle) -> Result<(), String> {
    win::execute_uac_action(app_handle, "uninstall")
}

// 计划任务控制方法 (Local 模式静默提权)
#[cfg(target_os = "windows")]
pub fn run_direct_tun_task(app_handle: &tauri::AppHandle) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    // 前置检查：即使计划任务存在，如果底层可执行文件丢失，也需要提示重新提权安装
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let target_svc_path = std::path::PathBuf::from(&program_data).join("Auroweave").join("bin").join("AuroDaemon.exe");
    if !target_svc_path.exists() {
        return Err("底层服务程序文件缺失，请重新执行一键安装提权组件".to_string());
    }

    // 写入当前主进程的 PID 供后台看门狗轮询退出
    let pid = std::process::id();
    let config_dir = crate::get_config_dir();
    let pid_file = config_dir.join("parent.pid");
    let _ = std::fs::write(pid_file, pid.to_string());
    
    // 生成 manifest.json 以供服务自更新
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
    
    let svc_hash = compute_sha256(&svc_path);
    let singbox_hash = compute_sha256(&singbox_path);
    
    let manifest = serde_json::json!({
        "svc_path": svc_path.to_string_lossy(),
        "svc_hash": svc_hash,
        "singbox_path": singbox_path.to_string_lossy(),
        "singbox_hash": singbox_hash
    });
    
    let _ = std::fs::write(manifest_file, manifest.to_string());

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

#[cfg(target_os = "windows")]
pub fn stop_direct_tun_task() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    log::info!("执行 schtasks /end /tn AuroweaveDirectTunTask");
    let mut cmd = std::process::Command::new("schtasks");
    cmd.arg("/end")
        .arg("/tn")
        .arg("AuroweaveDirectTunTask")
        .creation_flags(CREATE_NO_WINDOW);
        
    let _ = cmd.status();

    // 强杀所有可能残留的后台提权进程
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "AuroDaemon.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
        
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "sing-box.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();

    Ok(())
}

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
    
    // ASCII "Running" (7 bytes)
    let contains_running_en = bytes.windows(7).any(|w| w == b"Running");
    
    // GBK "正在运行" (8 bytes) -> 正=D5FD, 在=D4DA, 运=D4CB, 行=D0D0
    let gbk_running = [0xd5, 0xfd, 0xd4, 0xda, 0xd4, 0xcb, 0xd0, 0xd0];
    let contains_running_zh = bytes.windows(8).any(|w| w == gbk_running);
    
    // ASCII "Ready" (5 bytes)
    let contains_ready_en = bytes.windows(5).any(|w| w == b"Ready");
    
    // GBK "准备就绪" (8 bytes) -> 准=D7BC, 备=B1B8, 就=BED9, 绪=D0F7
    let gbk_ready = [0xd7, 0xbc, 0xb1, 0xb8, 0xbe, 0xd9, 0xd0, 0xf7];
    let contains_ready_zh = bytes.windows(8).any(|w| w == gbk_ready);
    
    // ASCII "Unknown" (7 bytes)
    let contains_unknown = bytes.windows(7).any(|w| w == b"Unknown");

    let running = contains_running_en || contains_running_zh || contains_ready_en || contains_ready_zh || contains_unknown;
    Ok(running)
}

#[cfg(target_os = "windows")]
pub fn query_singbox_process_running() -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let mut cmd = std::process::Command::new("tasklist");
    cmd.arg("/FO")
        .arg("CSV")
        .arg("/NH")
        .creation_flags(CREATE_NO_WINDOW);
        
    let output = cmd.output();
    
    if let Ok(out) = output {
        let stdout_str = String::from_utf8_lossy(&out.stdout);
        for line in stdout_str.lines() {
            let lower = line.to_lowercase();
            // 简单判断进程名中是否包含 sing-box
            if lower.contains("\"sing-box") {
                return true;
            }
        }
    }
    false
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
