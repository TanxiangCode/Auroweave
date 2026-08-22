/// sing-box sidecar 生命周期管理
/// 作者: TanXiang
///
/// SidecarManager 负责 sing-box 子进程的完整生命周期管理：
///
/// - **启动流程** (`start`):
///   1. 检查当前状态，避免重复启动
///   2. 通过 `resolve_binary_path` 多路径搜索定位 sing-box.exe
///   3. 使用 `tokio::process::Command` 拉起子进程，捕获 stdout/stderr
///   4. 等待 1.5 秒检测早期退出（配置错误、权限不足等）
///   5. Windows 上绑定到 Job Object，确保随主进程退出
///   6. 更新状态为 Running
///
/// - **停止流程** (`stop`):
///   1. 发送 kill 信号终止子进程
///   2. 调用 wait() 回收子进程资源，避免僵尸进程
///   3. 更新状态为 Stopped
///
/// - **状态查询** (`get_status`):
///   使用 try_lock 非阻塞获取状态，避免在状态查询时阻塞
///
/// 线程安全：所有共享字段使用 `tokio::sync::Mutex`，确保 guard 实现 Send，
/// 可以安全跨 await 边界传递。
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
    /// 标记进程是否通过 osascript 提权启动（macOS TUN 模式）
    /// 为 true 时 stop() 需通过 PID 文件 kill，而非 child.kill()
    is_external: Arc<Mutex<bool>>,
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
            is_external: Arc::new(Mutex::new(false)),
            #[cfg(target_os = "windows")]
            job,
        }
    }

    /// 非阻塞获取当前 sidecar 状态
    ///
    /// 使用 `try_lock` 而非 `lock().await`，避免在状态查询时阻塞调用方线程。
    /// 若锁被占用（例如正在执行 start/stop），返回错误状态而非等待。
    pub fn get_status(&self) -> SidecarStatus {
        self.status
            .try_lock()
            .map(|s| s.clone())
            .unwrap_or(SidecarStatus::Error("锁获取失败".to_string()))
    }

    /// 检查当前进程是否通过外部提权启动
    #[cfg(target_os = "macos")]
    pub async fn is_external(&self) -> bool {
        *self.is_external.lock().await
    }

    /// 仅更新状态为 Stopped，不实际终止进程
    pub async fn mark_stopped(&self) {
        *self.status.lock().await = SidecarStatus::Stopped;
    }

    /// 检查 macOS 下指定二进制文件是否已经配置 SUID root 权限
    #[cfg(target_os = "macos")]
    pub fn is_privileged_binary(path: &std::path::Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let is_root = meta.uid() == 0;
            let is_suid = (meta.mode() & 0o4000) != 0;
            is_root && is_suid
        } else {
            false
        }
    }

    /// 确保 macOS 下 sing-box 二进制文件具备 SUID root 特权（首次运行时请求一次管理员密码）
    #[cfg(target_os = "macos")]
    pub async fn ensure_privileged_binary(path: &std::path::Path) -> Result<(), AppError> {
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        };
        let abs_path = std::fs::canonicalize(&abs_path).unwrap_or(abs_path);

        if Self::is_privileged_binary(&abs_path) {
            info!("[sidecar] sing-box 已具备 SUID root 权限，直接免密拉起: {:?}", abs_path);
            return Ok(());
        }

        info!("[sidecar] sing-box 未具备 SUID 权限，请求一次性管理员权限赋权: {:?}", abs_path);
        let binary_str = abs_path.to_string_lossy().to_string();
        let shell_cmd = format!(
            "cd /tmp && chown root:admin '{}' && chmod +rx '{}' && chmod u+s '{}'",
            binary_str, binary_str, binary_str
        );
        let escaped_cmd = shell_cmd.replace('\\', "\\\\").replace('"', "\\\"");
        let applescript = format!(
            "do shell script \"{}\" with administrator privileges",
            escaped_cmd
        );

        let output = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(&applescript)
            .output()
            .await
            .map_err(|e| AppError::Sidecar(format!("执行 osascript 失败: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let err_msg = if stderr.contains("-128") || stderr.contains("User canceled") {
                "用户取消了管理员权限授权".to_string()
            } else {
                format!("特权赋权失败: {}", stderr.trim())
            };
            return Err(AppError::Sidecar(err_msg));
        }

        if Self::is_privileged_binary(&abs_path) {
            info!("[sidecar] sing-box SUID 赋权成功，后续启动 TUN 将永久免输密码");
            Ok(())
        } else {
            Err(AppError::Sidecar("特权赋权已执行但未能成功设置 SUID 标记".to_string()))
        }
    }

    /// 静默停止 sing-box 进程（不弹出 macOS 密码框）
    ///
    /// 用于应用退出场景：
    /// 直接通过子进程句柄 kill + wait 回收资源，确保退出后活动监视器零残留。
    pub async fn stop_silent(&self) -> Result<(), AppError> {
        let child = self.process.lock().await.take();

        if let Some(mut child) = child {
            let _ = child.kill().await;
            let _ = child.wait().await;
            info!("[sidecar] sing-box 子进程已静默停止并回收");
        }

        // 清理旧版本可能残留的 PID 文件与外部进程标记
        #[cfg(target_os = "macos")]
        {
            let pid_file = std::env::temp_dir().join("auroweave-singbox.pid");
            let _ = std::fs::remove_file(&pid_file);
        }
        *self.is_external.lock().await = false;

        *self.status.lock().await = SidecarStatus::Stopped;
        Ok(())
    }

    /// 启动 sing-box 子进程
    ///
    /// 完整启动流程：
    /// 1. 状态守卫：若已在运行则跳过，否则标记为 Starting
    /// 2. 定位二进制：通过 `resolve_binary_path` 多路径搜索
    /// 3. 特权检测（macOS）：若配置包含 TUN 入站，确保具备 SUID 权限（首次请求一次，之后永久免密）
    /// 4. 拉起进程：`tokio::process::Command` 启动子进程，管道捕获 stdout/stderr
    /// 5. 日志转发：spawn 异步任务将 stdout/stderr 逐行写入 tracing 日志
    /// 6. 早期退出检测：等待 1.5 秒后 try_wait，若已退出则收集 stderr 返回错误
    /// 7. Job Object 绑定（Windows）：确保子进程随主进程退出
    /// 8. 状态更新：标记为 Running
    pub async fn start(&self, config_path: &str) -> Result<(), AppError> {
        // ---- 阶段1: 状态守卫，避免重复启动 ----
        {
            let mut status = self.status.lock().await;
            if *status == SidecarStatus::Running {
                info!("sing-box 已在运行，跳过启动");
                return Ok(());
            }
            *status = SidecarStatus::Starting;
        }
        *self.is_external.lock().await = false;

        // ---- 阶段2: 定位 sing-box 可执行文件 ----
        let binary_path = Self::resolve_binary_path()?;
        info!("找到 sing-box 执行文件: {:?}", binary_path);

        // ---- 阶段2b: macOS TUN 模式 SUID 特权保障 ----
        #[cfg(target_os = "macos")]
        {
            let is_tun = std::fs::read_to_string(config_path)
                .map(|c| c.contains("\"type\": \"tun\"") || c.contains("\"type\":\"tun\""))
                .unwrap_or(false);
            if is_tun {
                if let Err(e) = Self::ensure_privileged_binary(&binary_path).await {
                    *self.status.lock().await = SidecarStatus::Stopped;
                    return Err(e);
                }
            }
        }

        info!("启动 sing-box: {:?} run -c {}", binary_path, config_path);

        // ---- 阶段3: 拉起子进程，管道捕获 stdout/stderr ----
        use std::process::Stdio;
        let mut child = tokio::process::Command::new(&binary_path)
            .arg("run")
            .arg("-c")
            .arg(config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                let err_msg = format!("拉起 sing-box 失败: {}", e);
                error!("{}", err_msg);
                AppError::Sidecar(err_msg)
            })?;

        // ---- 阶段4: 日志转发 ----
        let stdout = child.stdout.take();
        let stderr_lines_arc = Arc::new(Mutex::new(Vec::<String>::new()));
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

        // ---- 阶段5: 早期退出检测 ----
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

        match child.try_wait() {
            Ok(Some(exit_status)) => {
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
            Ok(None) => {}
            Err(e) => {
                warn!("检查 sing-box 进程状态时出现警告: {}", e);
            }
        }

        // ---- 阶段6: 存储子进程句柄 + Job Object 绑定 ----
        {
            let mut proc_guard = self.process.lock().await;
            *proc_guard = Some(child);

            #[cfg(target_os = "windows")]
            {
                if let Some(ref j) = self.job {
                    if let Some(ref p) = *proc_guard {
                        if let Some(handle) = p.raw_handle() {
                            let _ = j.assign_process(handle);
                        }
                    }
                }
            }
        }

        // ---- 阶段7: 标记为 Running，启动完成 ----
        *self.status.lock().await = SidecarStatus::Running;
        info!("sing-box 子进程启动成功");
        Ok(())
    }

    /// 兼容接口：通过 SUID 启动 sing-box（macOS TUN 模式直接调用 start）
    #[cfg(target_os = "macos")]
    pub async fn start_privileged(&self, config_path: &str) -> Result<(), AppError> {
        self.start(config_path).await
    }

    /// 兼容接口：重启 sing-box
    #[cfg(target_os = "macos")]
    pub async fn restart_privileged(&self, config_path: &str) -> Result<(), AppError> {
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        self.start(config_path).await
    }

    /// 停止 sing-box 子进程并回收资源
    pub async fn stop(&self) -> Result<(), AppError> {
        self.stop_silent().await
    }

    /// 系统从睡眠/休眠唤醒后重启 sing-box
    ///
    /// 唤醒后网络栈可能处于异常状态（TUN 网卡失联、DNS 缓存过期等），
    /// 需要先停止再重新启动 sing-box 以恢复正常代理。
    /// 中间留 500ms 间隔确保旧进程完全释放端口和资源。
    pub async fn handle_wake(&self, config_path: &str) -> Result<(), AppError> {
        warn!("系统从睡眠唤醒，重启 sing-box...");
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start(config_path).await
    }

    /// 多路径候选自动匹配 sing-box 二进制文件路径
    ///
    /// 搜索策略（按候选目录顺序遍历，选择修改时间最新的匹配文件）：
    /// 1. 开发环境相对路径：`src-tauri/sidecar-bin/<platform>`
    /// 2. 打包后相对路径：`sidecar-bin/<platform>`
    /// 3. 可执行文件同目录（安装后的标准位置）
    /// 4. `%ProgramData%\Auroweave\bin`（服务安装后的位置）
    ///
    /// 在每个候选目录中查找 `sing-box*` 前缀的可执行文件，
    /// 按文件修改时间选择最新版本（支持多版本共存场景）。
    pub fn resolve_binary_path() -> Result<PathBuf, AppError> {
        let mut candidate_dirs = Vec::new();

        // 候选目录1: 开发环境工作目录下的 sidecar-bin
        #[cfg(target_os = "windows")]
        {
            candidate_dirs.push(PathBuf::from("src-tauri/sidecar-bin/windows-x64"));
            candidate_dirs.push(PathBuf::from("sidecar-bin/windows-x64"));
        }

        #[cfg(target_os = "macos")]
        {
            candidate_dirs.push(PathBuf::from("src-tauri/sidecar-bin/macos-arm64"));
            candidate_dirs.push(PathBuf::from("src-tauri/sidecar-bin/macos-amd64"));
            candidate_dirs.push(PathBuf::from("sidecar-bin/macos-arm64"));
            candidate_dirs.push(PathBuf::from("sidecar-bin/macos-amd64"));
        }

        // 候选目录2: 可执行文件同目录（安装后的标准位置）
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                candidate_dirs.push(exe_dir.to_path_buf());
            }
        }

        // 候选目录3: 服务安装后的位置
        #[cfg(target_os = "windows")]
        {
            let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
            candidate_dirs.push(PathBuf::from(program_data).join("Auroweave").join("bin"));
        }
        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                candidate_dirs.push(PathBuf::from(home).join("Library").join("Application Support").join("Auroweave").join("bin"));
            }
        }

        // 遍历所有候选目录，按修改时间选择最新的 sing-box 可执行文件
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
                    let is_match = file_name.starts_with("sing-box")
                        && !file_name.ends_with(".tar.gz")
                        && !file_name.ends_with(".zip")
                        && !file_name.ends_with(".txt")
                        && !file_name.ends_with(".gitkeep");

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
            let abs_path = if path.is_absolute() {
                path
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(&path))
                    .unwrap_or(path)
            };
            let abs_path = std::fs::canonicalize(&abs_path).unwrap_or(abs_path);
            return Ok(abs_path);
        }

        let err_msg = "找不到 sing-box 二进制文件，请运行 download-sidecar 脚本或通过界面下载".to_string();
        error!("{}", err_msg);
        Err(AppError::Sidecar(err_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_privileged_binary_non_existent() {
        let fake_path = std::path::Path::new("/non/existent/path");
        #[cfg(target_os = "macos")]
        assert!(!SidecarManager::is_privileged_binary(fake_path));
    }
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}
