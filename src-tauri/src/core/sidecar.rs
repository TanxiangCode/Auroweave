/// sing-box sidecar 生命周期管理
/// 作者: TanXiang
use crate::error::AppError;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Debug, Clone, PartialEq)]
pub enum SidecarStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

/// 全部字段使用 tokio::sync::Mutex，确保 guard 实现 Send，可以安全跨 await 边界
pub struct SidecarManager {
    process: Arc<Mutex<Option<Child>>>,
    status: Arc<Mutex<SidecarStatus>>,
    #[cfg(target_os = "windows")]
    job: Option<crate::system::job::JobObject>,
}

impl SidecarManager {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        let job = crate::system::job::JobObject::create()
            .map_err(|e| error!("创建 Windows 作业对象失败: {}", e))
            .ok();

        Self {
            process: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(SidecarStatus::Stopped)),
            #[cfg(target_os = "windows")]
            job,
        }
    }

    pub fn get_status(&self) -> SidecarStatus {
        self.status
            .try_lock()
            .map(|s| s.clone())
            .unwrap_or(SidecarStatus::Error("锁获取失败".to_string()))
    }

    pub async fn start(&self, config_path: &str) -> Result<(), AppError> {
        {
            let mut status = self.status.lock().await;
            if *status == SidecarStatus::Running {
                info!("sing-box 已在运行，跳过启动");
                return Ok(());
            }
            *status = SidecarStatus::Starting;
        }

        let binary_path = Self::resolve_binary_path()?;
        info!("找到 sing-box 执行文件: {:?}", binary_path);
        info!("启动 sing-box: {:?} run -c {}", binary_path, config_path);

        use std::process::Stdio;
        let mut child = tokio::process::Command::new(&binary_path)
            .arg("run")
            .arg("-c")
            .arg(config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Sidecar(format!("拉起 sing-box 失败: {}", e)))?;

        // 取出 stdout / stderr 管道
        let stdout = child.stdout.take();

        // 捕获 stderr 供早期退出检测
        let stderr_lines_arc =
            Arc::new(Mutex::new(Vec::<String>::new()));
        if let Some(stderr) = child.stderr.take() {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let arc_clone = stderr_lines_arc.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    error!("[sing-box error] {}", line);
                    arc_clone.lock().await.push(line);
                }
            });
        }

        if let Some(stdout) = stdout {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    info!("[sing-box] {}", line);
                }
            });
        }

        // 等待 1.5 秒，检测进程是否因错误（权限不足等）提早退出
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

        match child.try_wait() {
            Ok(Some(exit_status)) => {
                // 进程已退出 —— 启动失败
                let mut err_msg = format!(
                    "sing-box 启动后立即退出，退出码: {:?}",
                    exit_status.code()
                );
                let lines = stderr_lines_arc.lock().await;
                if !lines.is_empty() {
                    err_msg = format!("{}\n详情: {}", err_msg, lines.join("\n"));
                }
                drop(lines);
                *self.status.lock().await = SidecarStatus::Stopped;
                return Err(AppError::Sidecar(err_msg));
            }
            Ok(None) => {
                // 进程仍在运行 —— 启动成功
            }
            Err(e) => {
                warn!("检查 sing-box 进程状态时出现警告: {}", e);
            }
        }

        // 存储子进程句柄
        {
            let mut proc_guard = self.process.lock().await;
            *proc_guard = Some(child);

            // Windows：绑定到 Job Object 使进程随主进程退出
            #[cfg(target_os = "windows")]
            {
                if let Some(ref j) = self.job {
                    if let Some(ref p) = *proc_guard {
                        if let Some(handle) = p.raw_handle() {
                            if let Err(e) = j.assign_process(handle) {
                                warn!("绑定 sing-box 进程到作业对象失败: {}", e);
                            } else {
                                info!("已成功将 sing-box 子进程绑定到作业对象");
                            }
                        }
                    }
                }
            }
        }

        *self.status.lock().await = SidecarStatus::Running;
        info!("sing-box 子进程启动成功");
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), AppError> {
        let child = self.process.lock().await.take();

        if let Some(mut child) = child {
            if let Err(e) = child.kill().await {
                warn!("停止 sing-box 进程时出现警告: {}", e);
            }
            info!("sing-box 进程已停止");
        }
        *self.status.lock().await = SidecarStatus::Stopped;
        Ok(())
    }

    pub async fn handle_wake(&self, config_path: &str) -> Result<(), AppError> {
        warn!("系统从睡眠唤醒，重启 sing-box...");
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start(config_path).await
    }

    /// 多路径候选自动匹配算法（防止工作目录或解压版本引起的找不到路径）
    pub fn resolve_binary_path() -> Result<PathBuf, AppError> {
        let mut candidate_dirs = Vec::new();

        #[cfg(target_os = "windows")]
        {
            candidate_dirs.push(PathBuf::from("src-tauri/sidecar-bin/windows-x64"));
            candidate_dirs.push(PathBuf::from("sidecar-bin/windows-x64"));
        }

        #[cfg(target_os = "macos")]
        {
            candidate_dirs.push(PathBuf::from("src-tauri/sidecar-bin/macos-universal"));
            candidate_dirs.push(PathBuf::from("sidecar-bin/macos-universal"));
        }

        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                candidate_dirs.push(exe_dir.to_path_buf());
            }
        }

        // 也查找 %ProgramData%\Auroweave\bin 目录 (服务安装后的路径)
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        candidate_dirs.push(PathBuf::from(program_data).join("Auroweave").join("bin"));

        let mut latest_path = None;
        let mut latest_time = std::time::SystemTime::UNIX_EPOCH;

        for dir in candidate_dirs {
            if !dir.exists() || !dir.is_dir() { continue; }
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() { continue; }
                    
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                    
                    #[cfg(target_os = "windows")]
                    let is_match = file_name.starts_with("sing-box") && file_name.ends_with(".exe");
                    
                    #[cfg(not(target_os = "windows"))]
                    let is_match = file_name.starts_with("sing-box") && !file_name.contains("."); // 避免匹配 .tar.gz 或其他压缩包

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

        let err_msg = "找不到 sing-box 二进制文件，请运行 download-sidecar 脚本或通过界面下载".to_string();
        error!("{}", err_msg);
        Err(AppError::Sidecar(err_msg))
    }
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}
