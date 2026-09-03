/// sing-box sidecar 生命周期管理
/// 作者: TanXiang
///
/// SidecarManager 负责 sing-box 子进程的完整生命周期管理：
///
/// - **启动流程** (`start`):
///   1. 检查当前状态，避免重复启动（Running/Starting 时幂等返回）
///   2. 通过 `resolve_binary_path` 多路径搜索定位 sing-box.exe
///   3. 使用 `tokio::process::Command` 拉起子进程，捕获 stdout/stderr
///   4. 轮询 `try_wait()` 检测早期退出（100ms 间隔、上限 15 次，可提前退出）
///   5. Windows 上绑定到 Job Object，确保随主进程退出
///   6. 更新状态为 Running
///
/// - **停止流程** (`stop`):
///   1. 发送 kill 信号终止子进程
///   2. 调用 wait()（带 3 秒超时兜底）回收子进程资源，避免僵尸进程与无限挂起
///   3. 更新状态为 Stopped
///
/// - **状态查询** (`get_status`):
///   使用 try_lock 非阻塞获取状态；锁被占用时返回最近一次缓存状态，而非误报错误
///
/// 并发安全设计：
/// - `lifecycle_lock` 串行化 start/stop 全流程（含 spawn 与早期退出检测等待），
///   避免并发 start 双 spawn、后启动者覆盖 process 槽位产生孤儿进程。
///   该锁可能被持有约 1.5s（早期退出检测），但 `get_status` 只依赖独立的
///   status 锁与 last_known_status 缓存，不会被 lifecycle_lock 阻塞。
/// - 所有共享字段使用 `tokio::sync::Mutex`，确保 guard 实现 Send，
///   可以安全跨 await 边界传递。
use crate::error::AppError;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// stderr 环形缓冲区容量上限（早期退出诊断时最多回看最近 200 行）
const STDERR_RING_CAP: usize = 200;

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
    /// 最近一次已知状态缓存：get_status 的 try_lock 失败时返回它，避免误报 Error
    last_known_status: Arc<Mutex<Option<SidecarStatus>>>,
    /// start/stop 串行锁：防止并发 start 双 spawn、并防止 stop 与 start 交错
    /// 产生孤儿进程。注意：此锁会被持有跨 await（最长约 1.5s 早期退出检测），
    /// 不能被 get_status 等查询路径依赖。
    lifecycle_lock: Arc<Mutex<()>>,
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
            last_known_status: Arc::new(Mutex::new(None)),
            lifecycle_lock: Arc::new(Mutex::new(())),
            #[cfg(target_os = "windows")]
            job,
        }
    }

    /// 更新状态并同时刷新最近已知状态缓存
    async fn set_status(&self, new_status: SidecarStatus) {
        *self.status.lock().await = new_status.clone();
        *self.last_known_status.lock().await = Some(new_status);
    }

    /// 非阻塞获取当前 sidecar 状态
    ///
    /// 使用 `try_lock` 而非 `lock().await`，避免在状态查询时阻塞调用方线程。
    /// 若锁被短暂占用（例如正在执行 start/stop 的状态变更），返回最近一次
    /// 已知状态缓存，而非误报 Error；缓存尚无数据时保守返回 Stopped。
    pub fn get_status(&self) -> SidecarStatus {
        match self.status.try_lock() {
            Ok(s) => s.clone(),
            Err(_) => match self.last_known_status.try_lock() {
                Ok(cached) => cached.clone().unwrap_or(SidecarStatus::Stopped),
                Err(_) => SidecarStatus::Stopped,
            },
        }
    }

    /// 仅更新状态为 Stopped，不实际终止进程
    pub async fn mark_stopped(&self) {
        self.set_status(SidecarStatus::Stopped).await;
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

    /// 校验二进制路径是否包含 shell 特殊字符
    ///
    /// 提权命令通过 `osascript` 拼接 shell 字符串执行，路径会被嵌入单引号内。
    /// 路径由内部 `resolve_binary_path`（canonicalize 后的绝对路径）生成，
    /// 正常仅包含字母数字与常规文件名字符；一旦出现引号、反引号、`$`、
    /// 换行等 shell 元字符，即视为异常来源（注入风险），直接拒绝执行。
    #[cfg(target_os = "macos")]
    fn validate_binary_path_for_shell(path_str: &str) -> Result<(), AppError> {
        let forbidden = [
            '\'', '"', '$', '`', '\\', '\n', '\r', '\t', '\0', ';', '|', '&', '(', ')',
            '<', '>', '{', '}', '[', ']', '*', '?', '~', '!', '#',
        ];
        if path_str.chars().any(|c| forbidden.contains(&c)) {
            return Err(AppError::Permission(format!(
                "sing-box 二进制路径包含 shell 特殊字符，已拒绝执行提权命令: {}",
                path_str
            )));
        }
        Ok(())
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

        // P0 安全：路径将被嵌入 osascript shell 命令，含特殊字符即拒绝执行
        Self::validate_binary_path_for_shell(&binary_str)?;

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
    /// wait 带 3 秒超时兜底：SUID root 进程可能拒绝非 root 父进程的 kill 信号
    /// （EPERM），此时 wait() 将永不返回，必须放弃等待避免退出流程无限挂起。
    pub async fn stop_silent(&self) -> Result<(), AppError> {
        // 与 start 串行：避免 stop 期间另一个 start 又拉起新进程
        let _lifecycle = self.lifecycle_lock.lock().await;

        let child = self.process.lock().await.take();

        if let Some(mut child) = child {
            if let Err(e) = child.kill().await {
                // 不再静默吞掉失败（如 SUID root 进程返回 EPERM），必须留痕
                error!("[sidecar] 向 sing-box 子进程发送 kill 信号失败: {}", e);
            }
            match tokio::time::timeout(
                std::time::Duration::from_secs(3),
                child.wait(),
            )
            .await
            {
                Ok(_) => {
                    info!("[sidecar] sing-box 子进程已停止并回收");
                }
                Err(_) => {
                    error!(
                        "[sidecar] sing-box 子进程 3 秒内未退出（root 进程可能拒绝了终止信号），\
                         进程已残留，需手动清理"
                    );
                }
            }
        }

        // 清理旧版本可能残留的 PID 文件
        #[cfg(target_os = "macos")]
        {
            let pid_file = std::env::temp_dir().join("auroweave-singbox.pid");
            let _ = std::fs::remove_file(&pid_file);
        }

        self.set_status(SidecarStatus::Stopped).await;
        Ok(())
    }

    /// 启动 sing-box 子进程
    ///
    /// 完整启动流程：
    /// 1. 状态守卫：若已在运行/启动中则幂等返回，否则标记为 Starting
    /// 2. 生命周期串行锁：确保同一时刻只有一个 start/stop 在执行，
    ///    并发 start 不会双 spawn、不会覆盖 process 槽位产生孤儿进程
    /// 3. 定位二进制：通过 `resolve_binary_path` 多路径搜索
    /// 4. 特权检测（macOS）：若配置包含 TUN 入站，确保具备 SUID 权限（首次请求一次，之后永久免密）
    /// 5. 拉起进程：`tokio::process::Command` 启动子进程，管道捕获 stdout/stderr
    /// 6. 日志转发：spawn 异步任务将 stdout/stderr 逐行写入 tracing 日志
    /// 7. 早期退出检测：每 100ms 轮询 `try_wait()`，上限 15 次（总 1.5s，可提前退出）
    /// 8. Job Object 绑定（Windows）：确保子进程随主进程退出
    /// 9. 状态更新：标记为 Running
    pub async fn start(&self, config_path: &str) -> Result<(), AppError> {
        // ---- 阶段1: 状态守卫（快速路径），避免重复启动 ----
        // 若另一个 start 正在进行（Starting），直接幂等返回，不与它竞争
        {
            let status = self.status.lock().await;
            match *status {
                SidecarStatus::Running => {
                    info!("sing-box 已在运行，跳过启动");
                    return Ok(());
                }
                SidecarStatus::Starting => {
                    info!("sing-box 正在启动中，跳过重复启动");
                    return Ok(());
                }
                _ => {}
            }
        }

        // ---- 阶段1b: 生命周期串行锁 ----
        // 该锁会跨 spawn 与早期退出检测（最长约 1.5s）持有；
        // get_status 不依赖此锁，因此不会阻塞状态查询。
        // 若等待期间另一个 start 已完成，锁内二次检查会幂等返回。
        let _lifecycle = self.lifecycle_lock.lock().await;
        {
            let status = self.status.lock().await;
            match *status {
                SidecarStatus::Running | SidecarStatus::Starting => {
                    info!("sing-box 已由并发请求启动，跳过本次启动");
                    return Ok(());
                }
                _ => {}
            }
        }
        self.set_status(SidecarStatus::Starting).await;

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
                    self.set_status(SidecarStatus::Stopped).await;
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
        // stderr 使用固定容量环形缓冲（VecDeque，上限 200 行），
        // 避免异常进程狂刷日志导致内存无上限增长
        let stdout = child.stdout.take();
        let stderr_lines_arc = Arc::new(Mutex::new(VecDeque::<String>::new()));
        if let Some(stderr) = child.stderr.take() {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let arc_clone = stderr_lines_arc.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    error!("[sing-box error] {}", line);
                    let mut buf = arc_clone.lock().await;
                    if buf.len() >= STDERR_RING_CAP {
                        buf.pop_front();
                    }
                    buf.push_back(line);
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
        // 每 100ms 轮询一次 try_wait，上限 15 次（总时长 1.5s 不变，
        // 但进程一旦提前退出即可立即返回，无需睡满全程）
        let mut early_exit: Option<std::process::ExitStatus> = None;
        for _ in 0..15 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            match child.try_wait() {
                Ok(Some(exit_status)) => {
                    early_exit = Some(exit_status);
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    warn!("检查 sing-box 进程状态时出现警告: {}", e);
                }
            }
        }

        if let Some(exit_status) = early_exit {
            let mut err_msg = format!(
                "sing-box 启动后立即退出，退出码: {:?}",
                exit_status.code()
            );
            let lines = stderr_lines_arc.lock().await;
            if !lines.is_empty() {
                err_msg = format!("{}\n详情: {}", err_msg, lines.iter().rev().take(30).rev().cloned().collect::<Vec<String>>().join("\n"));
            }
            drop(lines);
            self.set_status(SidecarStatus::Stopped).await;
            return Err(AppError::Sidecar(err_msg));
        }

        // ---- 阶段6: 存储子进程句柄 + Job Object 绑定 ----
        {
            let mut proc_guard = self.process.lock().await;
            // 防御性回收：正常流程 stop 已清空槽位，若此处仍有残留句柄
            // （历史异常路径遗留），先 kill 回收，避免覆盖槽位产生孤儿进程
            if let Some(mut stale) = proc_guard.take() {
                warn!("[sidecar] process 槽位存在残留句柄，启动前先回收旧进程");
                let _ = stale.kill().await;
                let _ = stale.wait().await;
            }
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

        // ---- 阶段6b: 进程退出监听 ----
        // 历史缺陷：Running 状态置位后无人监控子进程，内核崩溃/被系统杀死时
        // get_status 仍返回 Running，自愈体系（core_query_running 等）整体静默失效。
        // 此处以低频轮询 try_wait 检测退出（不能移动 Child：stop 需要 kill 句柄），
        // 退出时把状态回落 Stopped 并记录日志，供上层自愈逻辑感知。
        {
            let process = self.process.clone();
            let status = self.status.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_millis(800));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                loop {
                    interval.tick().await;
                    let exited = {
                        let mut guard = process.lock().await;
                        match guard.as_mut() {
                            Some(child) => matches!(child.try_wait(), Ok(Some(_))),
                            // 槽位被 stop() 清空：正常停机，结束监听
                            None => return,
                        }
                    };
                    if exited {
                        let cur = status.lock().await.clone();
                        if matches!(cur, SidecarStatus::Running) {
                            warn!("[sidecar] 检测到 sing-box 异常退出，状态回落 Stopped");
                            *status.lock().await = SidecarStatus::Stopped;
                            // last_known 缓存保持同步，避免 get_status 读到陈旧 Running
                        }
                        return;
                    }
                    let running = matches!(*status.lock().await, SidecarStatus::Running);
                    if !running {
                        return; // stop() 已正常处理，避免重复干预
                    }
                }
            });
        }

        // ---- 阶段7: 标记为 Running，启动完成 ----
        self.set_status(SidecarStatus::Running).await;
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

    /// shell 特殊字符路径必须被提权命令拒绝（注入防护）
    #[cfg(target_os = "macos")]
    #[test]
    fn test_validate_binary_path_rejects_special_chars() {
        assert!(SidecarManager::validate_binary_path_for_shell("/tmp/sing-box").is_ok());
        assert!(SidecarManager::validate_binary_path_for_shell("/tmp/si'ng-box").is_err());
        assert!(SidecarManager::validate_binary_path_for_shell("/tmp/sing\"box").is_err());
        assert!(SidecarManager::validate_binary_path_for_shell("/tmp/sing-$box").is_err());
        assert!(SidecarManager::validate_binary_path_for_shell("/tmp/sing`box").is_err());
        assert!(SidecarManager::validate_binary_path_for_shell("/tmp/sing\nbox").is_err());
    }
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}
