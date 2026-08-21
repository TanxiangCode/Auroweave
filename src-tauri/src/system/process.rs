/// 系统进程枚举与状态读取
/// 作者: TanXiang
///
/// 使用 sysinfo crate 获取进程列表。性能注意：
/// 使用 System::new() + refresh_processes() 而非 System::new_all()，
/// 避免采集不必要的 CPU/内存/磁盘信息。
use serde::{Deserialize, Serialize};
use sysinfo::System;

pub const PROCESS_NAME_DAEMON: &str = "aurodaemon";
pub const PROCESS_NAME_SINGBOX: &str = "sing-box";

/// 高效检查指定进程名的进程是否存活
///
/// 仅刷新进程列表，不采集 CPU/内存等额外信息，性能优于 System::new_all()
pub fn check_processes_running(keywords: &[&str]) -> std::collections::HashMap<String, bool> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
    
    let mut results = std::collections::HashMap::new();
    for kw in keywords {
        results.insert(kw.to_string(), false);
    }
    
    for process in sys.processes().values() {
        let name = process.name().to_string_lossy().to_lowercase();
        for kw in keywords {
            if name.contains(&kw.to_lowercase()) {
                results.insert(kw.to_string(), true);
            }
        }
    }
    
    results
}

/// 快捷检查单一进程是否存活
pub fn is_process_running(keyword: &str) -> bool {
    *check_processes_running(&[keyword]).get(keyword).unwrap_or(&false)
}

/// 根据可执行文件名强制结束进程 (Windows Only)
#[cfg(target_os = "windows")]
pub fn force_kill_process(exe_name: &str) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let mut cmd = std::process::Command::new("taskkill");
    cmd.args(["/F", "/IM", exe_name])
       .creation_flags(CREATE_NO_WINDOW);
       
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemProcess {
    pub pid: u32,
    pub name: String,
    pub exe_path: String,
    pub icon_base64: Option<String>,
}

#[cfg(target_os = "macos")]
extern "C" {
    fn macos_get_app_icon_base64(exe_path: *const std::os::raw::c_char) -> *mut std::os::raw::c_char;
    fn macos_free_string(ptr: *mut std::os::raw::c_char);
}

/// 提取指定进程的原生系统图标 (PNG Base64)
pub fn get_native_app_icon(exe_path: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        if exe_path.is_empty() {
            return None;
        }
        let c_path = std::ffi::CString::new(exe_path).ok()?;
        unsafe {
            let res_ptr = macos_get_app_icon_base64(c_path.as_ptr());
            if !res_ptr.is_null() {
                let s = std::ffi::CStr::from_ptr(res_ptr).to_str().ok().map(|s| s.to_string());
                macos_free_string(res_ptr);
                return s;
            }
        }
    }
    None
}

/// 获取当前系统活跃应用进程列表
///
/// 仅刷新进程列表，过滤系统基底进程（PID < 100），按名称去重排序。
pub fn get_active_processes() -> Vec<SystemProcess> {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All);

    let mut list = Vec::new();

    for (pid, process) in sys.processes() {
        let pid_u32 = pid.as_u32();
        // 过滤系统基底进程
        if pid_u32 < 100 {
            continue;
        }

        let name = process.name().to_string_lossy().to_string();
        let exe_path = process.exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

        if !name.is_empty() {
            let icon_base64 = get_native_app_icon(&exe_path);
            list.push(SystemProcess {
                pid: pid_u32,
                name,
                exe_path,
                icon_base64,
            });
        }
    }

    // 按进程名排序并去重
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));

    list
}

