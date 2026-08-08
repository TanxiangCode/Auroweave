/// sing-box 子进程管理（服务端侧）
/// 作者: TanXiang
///
/// CoreManager 运行在 AuroDaemon 系统服务/计划任务进程内，负责 sing-box 子进程的
/// 完整生命周期管理。与前端侧的 `SidecarManager` 不同，此处运行在提权环境（SYSTEM 或
/// 管理员权限），可创建 TUN 虚拟网卡等需要特权的 inbound。
///
/// - **启动流程** (`start`):
///   1. 状态守卫：若已在运行则先停止再重启
///   2. 定位 sing-box 二进制（多路径候选 + 按修改时间选最新）
///   3. 将配置内容写入缓存文件 `%ProgramData%\Auroweave\config\config.json`
///   4. 使用 `tokio::process::Command` 拉起子进程，Windows 上隐藏窗口
///   5. Windows Job Object 绑定：确保 AuroDaemon 退出时 sing-box 自动终止
///   6. 日志转发：spawn 异步任务将 stdout/stderr 写入 tracing 日志
///   7. 早期退出检测：等待 1.5 秒后 try_wait，判断是否因配置错误提前退出
///   8. 进程退出监听：spawn 后台任务监听 child.wait()，进程退出后自动更新状态
///
/// - **停止流程** (`stop`):
///   1. 取出 PID（take 后 pid 字段为 None）
///   2. 通过 `taskkill /F /PID` 强制终止进程
///   3. 等待 500ms 确保进程释放资源
///   4. 更新状态为 Stopped
///
/// 线程安全：pid 和 status 均使用 `tokio::sync::Mutex`，可安全跨 await 边界传递。
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

    /// 查询内核当前状态和 PID
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub async fn get_status(&self) -> (CoreStatus, Option<u32>) {
        let status = self.status.lock().await.clone();
        let pid = *self.pid.lock().await;
        (status, pid)
    }

    /// 启动（或重启）sing-box 子进程
    ///
    /// 完整启动流程：
    /// 1. 状态守卫：若已在运行则先停止再重启
    /// 2. 定位 sing-box 二进制路径
    /// 3. 将配置内容写入缓存文件
    /// 4. 拉起子进程（Windows 隐藏窗口）
    /// 5. Windows Job Object 绑定（进程级联终止）
    /// 6. 日志转发（stdout/stderr → tracing）
    /// 7. 早期退出检测（1.5 秒 try_wait）
    /// 8. 后台监听进程退出
    pub async fn start(&self, config_content: &str) -> Result<(), String> {
        // ---- 阶段1: 状态守卫，若已在运行则先停止再重启 ----
        let mut status_guard = self.status.lock().await;
        if *status_guard == CoreStatus::Running {
            info!("sing-box 已经在运行，准备重启...");
            drop(status_guard);
            self.stop().await?;
            status_guard = self.status.lock().await;
        }

        *status_guard = CoreStatus::Starting;

        // ---- 阶段2: 定位 sing-box 二进制路径 ----
        let binary_path = match Self::resolve_binary_path() {
            Ok(p) => p,
            Err(e) => {
                let err_msg = format!("解析 sing-box 二进制路径失败: {}", e);
                *status_guard = CoreStatus::Error(err_msg.clone());
                return Err(err_msg);
            }
        };

        info!("找到 sing-box 二进制文件: {:?}", binary_path);

        // ---- 阶段3: 将配置内容写入缓存文件 ----
        // 服务端将配置写入 %ProgramData%\Auroweave\config\config.json，供 sing-box 读取
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

        // ---- 阶段4: 拉起子进程（Windows 隐藏窗口） ----
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

        // ---- 阶段5: Windows Job Object 绑定 ----
        // 将 sing-box 进程绑定到 Job Object，确保 AuroDaemon 死后 sing-box 也必死
        // 故意泄漏 job 句柄，使其与 AuroDaemon 进程寿命绑定
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
                    // 这样 job object 就和当前的 AuroDaemon 进程寿命绑定在一起，
                    // AuroDaemon 退出时，操作系统会自动关闭 job 句柄，从而触发 KILL_ON_JOB_CLOSE 强制杀死 sing-box。
                }
            }
        }

        // ---- 阶段6: 日志转发（stdout/stderr → tracing） ----
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        // stderr 额外缓存到 Vec，供早期退出检测时拼装错误信息
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

        // ---- 阶段7: 早期退出检测（1.5 秒 try_wait） ----
        // 等待 1.5 秒后检查进程是否已退出（配置错误、端口冲突、权限不足等）
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

        // ---- 阶段8: 后台监听进程退出 ----
        // 将 child 的所有权转移到后台任务，避免多重 wait 卡死
        // 进程退出后自动清理 PID 和状态
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

    /// 停止 sing-box 子进程
    ///
    /// 停止流程：
    /// 1. 取出 PID（take 后 pid 字段为 None）
    /// 2. 通过 `taskkill /F /PID` 强制终止进程
    /// 3. 等待 500ms 确保进程释放资源
    /// 4. 更新状态为 Stopped
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

    /// 通过 PID 强制终止进程
    ///
    /// Windows: 使用 `taskkill /F /PID` 强制终止
    /// Unix: 使用 `kill -9 <pid>` 强制终止
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

    /// 解析 sing-box 二进制路径
    ///
    /// 注意：此实现比 utils::resolve_binary_path 更复杂，因为它：
    /// 1. 额外搜索 sidecar 候选路径（便于开发调试）
    /// 2. 按文件修改时间选择最新的版本（支持多版本共存场景）
    /// 如果不需要这些特性，应优先使用 utils::resolve_binary_path。
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
