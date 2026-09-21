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

/// ClashAPI 就绪探测：TCP 连上并收到任意 HTTP 响应即就绪
///
/// 用于启动等待加速（成功路径原来固定睡满 1.5s）。裸 tokio TCP + 手写
/// HEAD/GET，200ms 内无响应按未就绪处理——探测本身不阻塞超过一轮轮询间隔。
/// 鉴权失败（401）也视为"就绪"：服务已在监听即启动成功，鉴权由调用方处理。
async fn is_clash_api_ready(port: u16) -> bool {
    use tokio::io::AsyncWriteExt;
    let timeout = tokio::time::Duration::from_millis(200);
    match tokio::time::timeout(timeout, async {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await?;
        // 任意路径的 GET：只关心能否收到 HTTP 状态行，不解析 body
        let _ = stream.write_all(format!("GET /configs HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").as_bytes()).await;
        let mut buf = [0u8; 16];
        stream.readable().await?;
        let _ = stream.try_read(&mut buf);
        Ok::<bool, std::io::Error>(buf.starts_with(b"HTTP"))
    })
    .await
    {
        Ok(Ok(ready)) => ready,
        _ => false,
    }
}

/// 内核日志文件持久化（logs/singbox.log 追加写，超限轮转为 .old）
///
/// 简单轮转策略：写入前检查大小，超过 2MB 时把当前文件 rename 为 .old
/// （旧 .old 被覆盖），轮转失败不阻断写主日志。写入用 std::fs（行级小量追加，
/// spawn 的 tokio 任务里阻塞开销可忽略），互斥由调用方每行 append 的粒度保证
/// 足够（两任务交错至多导致行序微乱，不损文件完整性）。
#[derive(Clone)]
struct KernelLogFile {
    path: PathBuf,
}

impl KernelLogFile {
    const MAX_BYTES: u64 = 2 * 1024 * 1024;

    fn open() -> Self {
        let dir = crate::get_log_dir();
        let _ = std::fs::create_dir_all(&dir);
        Self { path: dir.join("singbox.log") }
    }

    fn append(&mut self, line: &str) {
        // 轮转检查（追加前，避免超限后再轮转丢最后一行）
        if let Ok(meta) = std::fs::metadata(&self.path) {
            if meta.len() >= Self::MAX_BYTES {
                let old = self.path.with_extension("log.old");
                let _ = std::fs::remove_file(&old);
                if std::fs::rename(&self.path, &old).is_err() {
                    // rename 失败（文件被占用等）：截断重来，保证日志不无限膨胀
                    let _ = std::fs::write(&self.path, "");
                }
            }
        }
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(f, "{}", line);
        }
    }
}

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
            '\'', '"', '$', '`', '\\', '\n', '\r', '\t', '\0', ';', '|', '&', '(',
            ')', '<', '>', '{', '}', '[', ']', '*', '?', '~', '!', '#',
        ];
        if path_str.chars().any(|c| forbidden.contains(&c)) {
            return Err(AppError::Permission(format!(
                "sing-box 二进制路径包含 shell 特殊字符，已拒绝执行提权命令: {}",
                path_str
            )));
        }
        Ok(())
    }

    /// 判断路径是否位于 macOS TCC 受保护目录（~/Documents、~/Desktop、~/Downloads）下
    ///
    /// TCC 对这三个目录的访问控制独立于传统 Unix 权限：即便提权后的 root 进程
    /// （osascript "with administrator privileges"）访问其中文件，也会被内核
    /// 按进程 TCC 标识拦截并返回 EPERM（Operation not permitted）。因此对位于
    /// 受保护目录内的二进制直接做 SUID 赋权必然失败，必须先复制到普通目录。
    /// `home` 参数注入便于单测；生产路径传当前用户 HOME。
    #[cfg(target_os = "macos")]
    fn is_tcc_protected_path_in(path: &std::path::Path, home: &std::path::Path) -> bool {
        let protected_components = ["Documents", "Desktop", "Downloads"];
        protected_components
            .iter()
            .any(|comp| path.starts_with(home.join(comp)))
    }

    #[cfg(target_os = "macos")]
    fn is_tcc_protected_path(path: &std::path::Path) -> bool {
        match std::env::var_os("HOME") {
            Some(home) => Self::is_tcc_protected_path_in(path, std::path::Path::new(&home)),
            None => false,
        }
    }

    /// 将二进制复制到 TCC 不受保护的暂存目录并返回新路径
    ///
    /// 开发环境下 sing-box 位于 `~/Documents/...` 内（TCC 受保护），直接对其
    /// chown/chmod 会因 TCC 拦截而失败。此函数将二进制复制到数据根目录的
    /// `bin/` 子目录（该目录本就是内核升级的安装目标与二进制搜索候选目录），
    /// 后续 SUID 赋权与进程拉起均针对副本进行。
    ///
    /// 复制策略：
    /// - 目标已存在且内容一致（大小一致且不早于源文件）时跳过复制，避免每次
    ///   启动都重写 80MB 文件
    /// - 复制采用"写临时文件 + rename"原子替换，目标正被运行中的 SUID 进程
    ///   占用时也能安全换 inode，失败时清理半成品
    #[cfg(target_os = "macos")]
    fn stage_binary_outside_tcc(path: &std::path::Path) -> Result<std::path::PathBuf, AppError> {
        Self::stage_binary_outside_tcc_in(path, &crate::get_data_root())
    }

    /// `stage_binary_outside_tcc` 的可测试核心：数据根目录由参数注入
    #[cfg(target_os = "macos")]
    fn stage_binary_outside_tcc_in(
        path: &std::path::Path,
        data_root: &std::path::Path,
    ) -> Result<std::path::PathBuf, AppError> {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "sing-box".to_string());
        let bin_dir = data_root.join("bin");
        std::fs::create_dir_all(&bin_dir).map_err(|e| {
            AppError::Io(format!("创建内核暂存目录失败: {} ({})", bin_dir.display(), e))
        })?;
        let target = bin_dir.join(&file_name);

        // 内容一致则直接复用。fs::copy 不保留 mtime，拷贝后目标 mtime 必然
        // >= 源；源文件被替换为新版本（mtime 更新）后会触发重新拷贝。
        let mtime_of = |m: &std::fs::Metadata| {
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        };
        let same = match (
            std::fs::metadata(path),
            std::fs::metadata(&target),
        ) {
            (Ok(src), Ok(dst)) => {
                matches!((mtime_of(&src), mtime_of(&dst)), (Some(s), Some(d)) if s <= d)
                    && src.len() == dst.len()
            }
            _ => false,
        };
        if same {
            return Ok(target);
        }

        info!(
            "[sidecar] sing-box 位于 TCC 受保护目录，复制到数据目录以允许提权: {:?} -> {:?}",
            path, target
        );
        let tmp_target = bin_dir.join(format!(".{}.tmp", file_name));
        std::fs::copy(path, &tmp_target).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_target);
            AppError::Io(format!(
                "复制内核到暂存目录失败: {} ({})。请检查磁盘空间与目录权限",
                tmp_target.display(),
                e
            ))
        })?;
        if let Err(e) = std::fs::rename(&tmp_target, &target) {
            let _ = std::fs::remove_file(&tmp_target);
            return Err(AppError::Io(format!(
                "内核暂存副本替换失败: {} ({})",
                target.display(),
                e
            )));
        }
        // 确保普通用户可执行（fs::copy 会保留源权限，但目标目录可能是新建的）
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&target) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&target, perms);
            }
        }
        Ok(target)
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
            "chown root:admin '{}' && chmod +rx '{}' && chmod u+s '{}'",
            binary_str, binary_str, binary_str
        );
        let escaped_cmd = shell_cmd.replace('\\', "\\\\").replace('"', "\\\"");
        let applescript = format!(
            "do shell script \"{}\" with administrator privileges",
            escaped_cmd
        );

        // 关键修复：osascript 子进程的 CWD 若位于 TCC 受保护目录（如 ~/Documents，
        // 开发环境从仓库目录启动时即是如此），提权后的 root shell 在初始化阶段
        // 就会因 getcwd() 被拒而报 shell-init 错误（cd 补丁无效——失败发生在
        // shell 执行任何命令之前）。显式切换到 /private/tmp 根治。
        let mut cmd = tokio::process::Command::new("osascript");
        cmd.arg("-e").arg(&applescript);
        cmd.current_dir("/private/tmp");

        let output = cmd
            .output()
            .await
            .map_err(|e| AppError::Sidecar(format!("执行 osascript 失败: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let err_msg = if stderr.contains("-128") || stderr.contains("User canceled") {
                "用户取消了管理员权限授权".to_string()
            } else {
                let mut msg = format!("特权赋权失败: {}", stderr.trim());
                if stderr.contains("Operation not permitted") {
                    msg.push_str(
                        "\n提示: 目标路径可能位于 macOS 隐私保护目录（~/Documents、~/Desktop、\
                         ~/Downloads，管理员权限也无法修改其中文件）或系统只读卷上。\
                         若从上述目录运行本应用，请在 系统设置 → 隐私与安全性 →\
                         完全磁盘访问权限 中为运行环境（终端/IDE）授权。",
                    );
                }
                msg
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
        // 二进制若位于 TCC 受保护目录（开发环境仓库在 ~/Documents 下、或用户
        // 从 ~/Downloads 直接运行应用），root 也无法对其 chown/chmod，必须先
        // 复制到数据目录的 bin/ 再对副本赋权与拉起
        #[cfg(target_os = "macos")]
        let mut binary_path = binary_path;
        #[cfg(target_os = "macos")]
        {
            let is_tun = std::fs::read_to_string(config_path)
                .map(|c| c.contains("\"type\": \"tun\"") || c.contains("\"type\":\"tun\""))
                .unwrap_or(false);
            if is_tun {
                if Self::is_tcc_protected_path(&binary_path) {
                    binary_path = Self::stage_binary_outside_tcc(&binary_path)?;
                }
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
        // 内核日志文件持久化：追加写入 logs/singbox.log，超过 2MB 轮转为 .old
        // （日志页 WS 流是内存态刷新即丢，落盘后崩溃/拒载问题可事后排查）
        let kernel_log = KernelLogFile::open();
        if let Some(stderr) = child.stderr.take() {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let arc_clone = stderr_lines_arc.clone();
            let mut log_file = kernel_log.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    error!("[sing-box error] {}", line);
                    log_file.append(&format!("[ERR] {}", line));
                    let mut buf = arc_clone.lock().await;
                    if buf.len() >= STDERR_RING_CAP {
                        buf.pop_front();
                    }
                    buf.push_back(line);
                }
            });
        }

        if let Some(stdout) = stdout {
            let mut log_file = kernel_log.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    info!("[sing-box] {}", line);
                    log_file.append(&format!("[INF] {}", line));
                }
            });
        }

        // ---- 阶段5: 早期退出检测 + 就绪加速 ----
        // 语义：1.5s 内进程死亡才算启动失败；成功路径原来必须睡满 1.5s。
        // 优化：每 100ms 轮询时顺带探测 ClashAPI 是否就绪（GET /configs），
        // 就绪即认为启动成功并提前返回（sing-box 实际 exec→监听通常 <300ms），
        // 拉起耗时从固定 1.5s 压缩到 ~200-400ms；未就绪继续轮询，保底
        // 语义（1.5s 死亡窗口）完全不变。
        let clash_port = crate::core::clash_api::get_clash_api_port();
        let mut early_exit: Option<std::process::ExitStatus> = None;
        for _ in 0..15 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            match child.try_wait() {
                Ok(Some(exit_status)) => {
                    early_exit = Some(exit_status);
                    break;
                }
                Ok(None) => {
                    // ClashAPI 就绪探测：成功即提前完成启动（无 HTTP 客户端池
                    // 依赖——裸 tokio TCP + 手写 GET，避免在 sidecar 模块引入 reqwest）
                    if is_clash_api_ready(clash_port).await {
                        info!("[sidecar] ClashAPI 已就绪，提前完成启动等待");
                        break;
                    }
                }
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

    /// TCC 受保护目录检测：~/Documents、~/Desktop、~/Downloads 内为 true，
    /// 同名前缀目录（如 ~/Documents-backup）不误伤
    #[cfg(target_os = "macos")]
    #[test]
    fn test_is_tcc_protected_path() {
        let home = std::path::Path::new("/Users/tester");
        assert!(SidecarManager::is_tcc_protected_path_in(
            &std::path::Path::new("/Users/tester/Documents/Git/app/bin/sing-box"),
            home
        ));
        assert!(SidecarManager::is_tcc_protected_path_in(
            &std::path::Path::new("/Users/tester/Downloads/sing-box"),
            home
        ));
        assert!(SidecarManager::is_tcc_protected_path_in(
            &std::path::Path::new("/Users/tester/Desktop/app"),
            home
        ));
        // 同名前缀目录不算受保护（starts_with 是路径组件语义）
        assert!(!SidecarManager::is_tcc_protected_path_in(
            &std::path::Path::new("/Users/tester/Documents-backup/sing-box"),
            home
        ));
        // 数据目录不受保护
        assert!(!SidecarManager::is_tcc_protected_path_in(
            &std::path::Path::new("/Users/tester/Library/Application Support/Auroweave/bin/sing-box"),
            home
        ));
        // 系统路径不受保护
        assert!(!SidecarManager::is_tcc_protected_path_in(
            &std::path::Path::new("/usr/local/bin/sing-box"),
            home
        ));
        assert!(!SidecarManager::is_tcc_protected_path_in(
            &std::path::Path::new("/private/tmp/sing-box"),
            home
        ));
    }

    /// 暂存复制：目标位于数据目录 bin/ 下、内容一致时跳过复制、
    /// 源更新后重新复制
    #[cfg(target_os = "macos")]
    #[test]
    fn test_stage_binary_outside_tcc() {
        let tmp = std::env::temp_dir().join(format!(
            "auroweave-tcc-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("创建测试目录失败");

        // 数据根目录注入为临时目录，避免污染真实 ~/Library 数据目录
        let data_root = tmp.join("data-root");

        let src = tmp.join("fake-sing-box");
        std::fs::write(&src, b"v1").expect("写源文件失败");

        let staged = SidecarManager::stage_binary_outside_tcc_in(&src, &data_root)
            .expect("暂存失败");
        assert_eq!(staged, data_root.join("bin").join("fake-sing-box"));
        assert_eq!(std::fs::read(&staged).unwrap(), b"v1");
        // 半成品临时文件不残留
        assert!(!bin_dir_of(&data_root).join(".fake-sing-box.tmp").exists());

        // 副本 mtime 必然 >= 源（fs::copy 不保留源 mtime）→ 跳过复制复用副本
        let staged2 = SidecarManager::stage_binary_outside_tcc_in(&src, &data_root)
            .expect("第二次暂存失败");
        assert_eq!(staged2, staged);

        // 源更新（mtime 变新）→ 重新复制
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&src, b"v2-longer").expect("更新源文件失败");
        let staged3 = SidecarManager::stage_binary_outside_tcc_in(&src, &data_root)
            .expect("第三次暂存失败");
        assert_eq!(staged3, staged);
        assert_eq!(std::fs::read(&staged3).unwrap(), b"v2-longer");
        assert!(!bin_dir_of(&data_root).join(".fake-sing-box.tmp").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[cfg(target_os = "macos")]
    fn bin_dir_of(data_root: &std::path::Path) -> std::path::PathBuf {
        data_root.join("bin")
    }
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}
