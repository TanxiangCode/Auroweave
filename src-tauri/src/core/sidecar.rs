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

    /// 启动 sing-box 子进程
    ///
    /// 完整启动流程：
    /// 1. 状态守卫：若已在运行则跳过，否则标记为 Starting
    /// 2. 定位二进制：通过 `resolve_binary_path` 多路径搜索
    /// 3. 拉起进程：`tokio::process::Command` 启动子进程，管道捕获 stdout/stderr
    /// 4. 日志转发：spawn 异步任务将 stdout/stderr 逐行写入 tracing 日志
    /// 5. 早期退出检测：等待 1.5 秒后 try_wait，若已退出则收集 stderr 返回错误
    /// 6. Job Object 绑定（Windows）：确保子进程随主进程退出
    /// 7. 状态更新：标记为 Running
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

        // ---- 阶段2: 定位 sing-box 可执行文件 ----
        let binary_path = Self::resolve_binary_path()?;
        info!("找到 sing-box 执行文件: {:?}", binary_path);
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
            .map_err(|e| AppError::Sidecar(format!("拉起 sing-box 失败: {}", e)))?;

        // ---- 阶段4: 日志转发 ----
        // 取出 stdout / stderr 管道，spawn 异步任务逐行写入 tracing 日志
        let stdout = child.stdout.take();

        // stderr 额外缓存到 Vec，供早期退出检测时拼装错误信息
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

        // ---- 阶段5: 早期退出检测 ----
        // 等待 1.5 秒后检查进程是否已退出（配置错误、端口冲突、权限不足等）
        // 若进程存活则说明启动成功；若已退出则收集 stderr 行拼装错误详情
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

        match child.try_wait() {
            Ok(Some(exit_status)) => {
                // 进程已退出 —— 启动失败，拼装 stderr 详情返回错误
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

        // ---- 阶段6: 存储子进程句柄 + Job Object 绑定 ----
        {
            let mut proc_guard = self.process.lock().await;
            *proc_guard = Some(child);

            // Windows：将子进程绑定到 Job Object，确保主进程退出时子进程自动终止
            // 防止异常退出后 sing-box 成为孤儿进程继续占用端口
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

        // ---- 阶段7: 标记为 Running，启动完成 ----
        *self.status.lock().await = SidecarStatus::Running;
        info!("sing-box 子进程启动成功");
        Ok(())
    }

    /// 停止 sing-box 子进程并回收资源
    ///
    /// 停止流程：
    /// 1. 取出子进程句柄（take 后 process 字段为 None）
    /// 2. 发送 kill 信号终止进程
    /// 3. 调用 wait() 回收子进程资源，避免僵尸进程
    /// 4. 更新状态为 Stopped
    pub async fn stop(&self) -> Result<(), AppError> {
        let child = self.process.lock().await.take();

        if let Some(mut child) = child {
            // 先发送 kill 信号，再 wait 回收子进程，避免僵尸进程
            if let Err(e) = child.kill().await {
                warn!("停止 sing-box 进程时出现警告: {}", e);
            }
            // 等待进程完全退出并回收资源，防止产生僵尸进程
            if let Err(e) = child.wait().await {
                warn!("等待 sing-box 进程退出时出现警告: {}", e);
            }
            info!("sing-box 进程已停止");
        }
        *self.status.lock().await = SidecarStatus::Stopped;
        Ok(())
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
            candidate_dirs.push(PathBuf::from("src-tauri/sidecar-bin/macos-universal"));
            candidate_dirs.push(PathBuf::from("sidecar-bin/macos-universal"));
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
