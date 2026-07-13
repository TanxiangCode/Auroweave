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
            Ok(dir) => dir.join("resources").join("auroweave-svc.exe"),
            Err(_) => std::path::PathBuf::new(),
        };

        if !svc_path.exists() {
            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {
                    let debug_path = exe_dir.join("auroweave-svc.exe");
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
pub fn install_service_uac(app_handle: &tauri::AppHandle) -> Result<(), String> {
    win::execute_uac_action(app_handle, "install")
}

#[cfg(target_os = "windows")]
pub fn uninstall_service_uac(app_handle: &tauri::AppHandle) -> Result<(), String> {
    win::execute_uac_action(app_handle, "uninstall")
}

// 计划任务控制方法 (Direct 模式静默提权)
#[cfg(target_os = "windows")]
pub fn run_direct_tun_task() -> Result<(), String> {
    tracing::info!("执行 schtasks /run /tn AuroweaveDirectTunTask");
    let status = std::process::Command::new("schtasks")
        .arg("/run")
        .arg("/tn")
        .arg("AuroweaveDirectTunTask")
        .status()
        .map_err(|e| format!("无法启动 schtasks 命令: {}", e))?;
    if !status.success() {
        return Err("启动提权计划任务失败，请确认是否已成功安装服务（静默任务随服务一并安装）".to_string());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn stop_direct_tun_task() -> Result<(), String> {
    tracing::info!("执行 schtasks /end /tn AuroweaveDirectTunTask");
    let _ = std::process::Command::new("schtasks")
        .arg("/end")
        .arg("/tn")
        .arg("AuroweaveDirectTunTask")
        .status();
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn query_direct_tun_task_running() -> Result<bool, String> {
    let output = std::process::Command::new("schtasks")
        .arg("/query")
        .arg("/tn")
        .arg("AuroweaveDirectTunTask")
        .arg("/fo")
        .arg("CSV")
        .arg("/nh")
        .output()
        .map_err(|e| format!("查询计划任务状态失败: {}", e))?;
    if !output.status.success() {
        return Ok(false);
    }
    let output_str = String::from_utf8_lossy(&output.stdout);
    // 包含 "Running", "正在运行" 或刚刚启动可能处于的 "Unknown" 状态皆判定为正在运行
    let running = output_str.contains("Running") 
        || output_str.contains("正在运行") 
        || output_str.contains("Unknown")
        || output_str.contains("Ready"); // 部分系统上可能在任务活跃时也包含就绪指示
    Ok(running)
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
pub fn install_service_uac(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    Err("当前平台不支持系统服务模式".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn uninstall_service_uac(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    Err("当前平台不支持系统服务模式".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn run_direct_tun_task() -> Result<(), String> {
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
