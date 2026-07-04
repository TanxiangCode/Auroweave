/// 系统进程枚举与状态读取
/// 作者: TanXiang
use serde::{Deserialize, Serialize};
use sysinfo::System;

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
