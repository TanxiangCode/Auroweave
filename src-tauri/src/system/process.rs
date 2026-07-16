/// 系统进程枚举与状态读取
/// 作者: TanXiang
use serde::{Deserialize, Serialize};
use sysinfo::System;

pub const PROCESS_NAME_DAEMON: &str = "aurodaemon";
pub const PROCESS_NAME_SINGBOX: &str = "sing-box";

/// 高效检查指定进程名的进程是否存活
pub fn check_processes_running(keywords: &[&str]) -> std::collections::HashMap<String, bool> {
    use sysinfo::{ProcessRefreshKind, RefreshKind};
    let mut sys = System::new_with_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::new())
    );
    
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
}

/// 获取当前系统活跃应用进程列表
pub fn get_active_processes() -> Vec<SystemProcess> {
    let mut sys = System::new_all();
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
            list.push(SystemProcess {
                pid: pid_u32,
                name,
                exe_path,
            });
        }
    }

    // 按进程名排序并去重
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));

    list
}
