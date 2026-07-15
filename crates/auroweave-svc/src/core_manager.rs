/// sing-box 子进程管理
/// 作者: TanXiang
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use tracing::{error, info, warn};

#[derive(Debug, Clone, PartialEq)]
pub enum CoreStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

pub struct CoreManager {
    pid: Arc<Mutex<Option<u32>>>,
    status: Arc<Mutex<CoreStatus>>,
}

impl CoreManager {
    pub fn new() -> Self {
        Self {
            pid: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(CoreStatus::Stopped)),
        }
    }

    pub async fn get_status(&self) -> (CoreStatus, Option<u32>) {
        let status = self.status.lock().await.clone();
        let pid = *self.pid.lock().await;
        (status, pid)
    }

    pub async fn start(&self, config_content: &str) -> Result<(), String> {
        let mut status_guard = self.status.lock().await;
        if *status_guard == CoreStatus::Running {
            info!("sing-box 已经在运行，准备重启...");
            drop(status_guard);
            self.stop().await?;
            status_guard = self.status.lock().await;
        }

        *status_guard = CoreStatus::Starting;

        // 获取可执行文件路径
        let binary_path = match Self::resolve_binary_path() {
            Ok(p) => p,
            Err(e) => {
                let err_msg = format!("解析 sing-box 二进制路径失败: {}", e);
                *status_guard = CoreStatus::Error(err_msg.clone());
                return Err(err_msg);
            }
        };

        info!("找到 sing-box 二进制文件: {:?}", binary_path);

        // 写入配置缓存文件
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        let cache_dir = PathBuf::from(program_data).join("Auroweave").join("config");
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            let err_msg = format!("创建服务缓存目录失败: {}", e);
            *status_guard = CoreStatus::Error(err_msg.clone());
            return Err(err_msg);
        }
        let config_path = cache_dir.join("config.json");
        let has_tun = config_content.contains("\"type\": \"tun\"") || config_content.contains("\"type\":\"tun\"");
        info!("正在写入服务缓存配置文件，总长度: {}, 是否携带 TUN inbound: {}", config_content.len(), has_tun);
        if let Err(e) = std::fs::write(&config_path, config_content) {
            let err_msg = format!("写入配置文件失败: {}", e);
            *status_guard = CoreStatus::Error(err_msg.clone());
            return Err(err_msg);
        }

        info!("启动 sing-box: {:?} run -c {:?}, 携带 TUN: {}", binary_path, config_path, has_tun);

        use std::process::Stdio;
        let mut cmd = tokio::process::Command::new(&binary_path);
        cmd.arg("run")
            .arg("-c")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
            
        #[cfg(target_os = "windows")]
        {

            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = match cmd.spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("拉起 sing-box 进程失败: {}", e);
                *status_guard = CoreStatus::Error(err_msg.clone());
                return Err(err_msg);
            }
        };

        // 将 sing-box 进程绑定到 Windows Job Object，确保 auroweave-svc 死后 sing-box 也必死
        #[cfg(target_os = "windows")]
        if let Some(child_pid) = child.id() {
            unsafe {
                use windows_sys::Win32::System::JobObjects::{CreateJobObjectW, AssignProcessToJobObject, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE};
                use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};
                use windows_sys::Win32::Foundation::CloseHandle;
                
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job != 0 {
                    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                    let success = SetInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        &info as *const _ as *const std::ffi::c_void,
                        std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32
                    );
                    if success != 0 {
                        let proc_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, child_pid);
                        if proc_handle != 0 {
                            AssignProcessToJobObject(job, proc_handle);
                            CloseHandle(proc_handle);
                        }
                    }
                    // 注意：这里我们故意泄漏（不 Close） job 句柄。
                    // 这样 job object 就和当前的 auroweave-svc 进程寿命绑定在一起，
                    // auroweave-svc 退出时，操作系统会自动关闭 job 句柄，从而触发 KILL_ON_JOB_CLOSE 强制杀死 sing-box。
                }
            }
        }

        // 取出 stdout/stderr 管道输出日志
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stderr_lines_arc = Arc::new(Mutex::new(Vec::<String>::new()));
        if let Some(stderr_stream) = stderr {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let arc_clone = stderr_lines_arc.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr_stream).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    error!("[sing-box error] {}", line);
                    arc_clone.lock().await.push(line);
                }
            });
        }

        if let Some(stdout_stream) = stdout {
            use tokio::io::{AsyncBufReadExt, BufReader};
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout_stream).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    info!("[sing-box] {}", line);
                }
            });
        }

        // 等待 1.5 秒，检测进程是否因配置错误或权限不足提前退出
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

        match child.try_wait() {
            Ok(Some(exit_status)) => {
                let mut err_msg = format!("sing-box 启动后立即退出，退出码: {:?}", exit_status.code());
                let lines = stderr_lines_arc.lock().await;
                if !lines.is_empty() {
                    err_msg = format!("{}\n详情: {}", err_msg, lines.join("\n"));
                }
                *status_guard = CoreStatus::Stopped;
                return Err(err_msg);
            }
            Ok(None) => {
                // 仍在运行
            }
            Err(e) => {
                warn!("检查进程状态错误: {}", e);
            }
        }

        let pid_val = child.id().unwrap_or(0);
        *self.pid.lock().await = Some(pid_val);
        *status_guard = CoreStatus::Running;

        let pid_clone = self.pid.clone();
        let status_clone = self.status.clone();
        
        // 开启监听进程退出的任务，将 child 的所有权直接转移进去，避免任何多重 wait 卡死
        tokio::spawn(async move {
            info!("开始监听 sing-box 进程退出 (PID: {})", pid_val);
            match child.wait().await {
                Ok(status) => {
                    info!("sing-box 进程 (PID: {}) 退出，状态为: {:?}", pid_val, status);
                }
                Err(e) => {
                    error!("监听进程 (PID: {}) 退出时发生异常: {}", pid_val, e);
                }
            }
            // 清理 PID 和状态
            *pid_clone.lock().await = None;
            *status_clone.lock().await = CoreStatus::Stopped;
        });

        info!("sing-box 托管进程启动成功，PID: {}", pid_val);
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let pid_opt = {
            let mut pid_guard = self.pid.lock().await;
            pid_guard.take()
        };

        if let Some(pid) = pid_opt {
            info!("正在终止 sing-box 进程 (PID: {})...", pid);
            Self::kill_process_by_pid(pid);
            
            // 简单等待一下以确保进程释放
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            info!("sing-box 进程 (PID: {}) 终止命令已发出", pid);
        }

        *self.status.lock().await = CoreStatus::Stopped;
        Ok(())
    }

    fn kill_process_by_pid(pid: u32) {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let _ = std::process::Command::new("taskkill")
                .arg("/F")
                .arg("/PID")
                .arg(pid.to_string())
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg(pid.to_string())
                .status();
        }
    }

    fn resolve_binary_path() -> Result<PathBuf, String> {
        let mut candidate_dirs = Vec::new();

        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                candidate_dirs.push(exe_dir.to_path_buf());
                // 尝试寻找 sidecar 候选路径（便于开发调试服务）
                candidate_dirs.push(exe_dir.join("../../../src-tauri/sidecar-bin/windows-x64"));
            }
        }

        let mut latest_path = None;
        let mut latest_time = std::time::SystemTime::UNIX_EPOCH;

        for dir in candidate_dirs {
            if !dir.exists() || !dir.is_dir() { continue; }
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() { continue; }
                    
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                    
                    let is_match = file_name.starts_with("sing-box") && file_name.ends_with(".exe");

                    if is_match {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            if let Ok(modified) = metadata.modified() {
                                if modified > latest_time {
                                    latest_time = modified;
                                    latest_path = Some(path);
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(path) = latest_path {
            return Ok(path);
        }

        Err("在候选目录下找不到任何 sing-box 可执行二进制文件".to_string())
    }
}
