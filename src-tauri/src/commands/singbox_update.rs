/// Sing-box 内核版本检测、在线升级与热替换模块
/// 作者: TanXiang
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tracing::info;


/// 内核升级阶段（前端据此渲染按钮文案与进度条粒度）
///
/// 分阶段而非单一百分比：下载只是升级的一小段，真正耗时且用户无感知的是
/// 停止内核 / 替换文件 / 重新拉起。不分阶段会让进度条在 70% 之后长时间静止，
/// 用户无法区分"正在解压"与"已卡死"。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SingboxUpdateStage {
    /// 空闲（无升级任务在途）
    Idle,
    /// 准备中（校验下载地址、初始化客户端）
    Preparing,
    /// 下载中
    Downloading,
    /// 校验升级包完整性
    Verifying,
    /// 解压中
    Extracting,
    /// 停止运行中的内核
    StoppingCore,
    /// 替换内核二进制文件
    Replacing,
    /// 重新拉起内核
    Restarting,
    /// 升级完成
    Done,
    /// 升级失败
    Failed,
}

impl SingboxUpdateStage {
    /// 该阶段是否代表任务已终结（成功或失败）
    pub fn is_terminal(self) -> bool {
        matches!(self, SingboxUpdateStage::Done | SingboxUpdateStage::Failed)
    }
}

/// 内核升级实时进度（事件负载 + 状态查询返回体）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SingboxUpdateProgress {
    pub stage: SingboxUpdateStage,
    /// 整体进度 0-100（下载段映射到 0-70，其余阶段为固定锚点）
    pub percent: u8,
    /// 阶段描述（可直接展示给用户的中文文案）
    pub message: String,
    /// 已下载字节数
    pub downloaded: u64,
    /// 升级包总字节数（服务端未返回 Content-Length 时为 0）
    pub total: u64,
    /// 任务是否已终结（成功或失败）
    pub finished: bool,
    /// 成功与否：仅在 finished 为 true 时有值
    pub success: Option<bool>,
    /// 失败原因：仅 stage == failed 时有值
    pub error: Option<String>,
}

impl SingboxUpdateProgress {
    fn idle() -> Self {
        Self {
            stage: SingboxUpdateStage::Idle,
            percent: 0,
            message: String::new(),
            downloaded: 0,
            total: 0,
            finished: false,
            success: None,
            error: None,
        }
    }
}

/// 升级进度事件名（前端 listen 订阅）
pub const CORE_UPDATE_PROGRESS_EVENT: &str = "core-update-progress";

/// 下载阶段占整体进度的权重上限：下载结束后余下 30% 留给校验/解压/停核/替换/重启
const DOWNLOAD_PERCENT_CAP: u64 = 70;

/// 下载进度事件节流间隔：过密的事件会挤占 IPC 通道并触发前端无谓重排
const DOWNLOAD_EMIT_INTERVAL: Duration = Duration::from_millis(120);

static UPDATE_PROGRESS: OnceLock<std::sync::RwLock<SingboxUpdateProgress>> = OnceLock::new();

/// 获取全局进度槽位：后端持有真值源，前端重挂载后可回查，避免"切页后进度归零"
fn progress_slot() -> &'static std::sync::RwLock<SingboxUpdateProgress> {
    UPDATE_PROGRESS.get_or_init(|| std::sync::RwLock::new(SingboxUpdateProgress::idle()))
}

/// 读取当前进度快照（锁内仅做拷贝，绝不跨 await 持锁）
fn read_progress() -> SingboxUpdateProgress {
    progress_slot()
        .read()
        .map(|g| g.clone())
        .unwrap_or_else(|_| SingboxUpdateProgress::idle())
}

/// 写入全局槽位并广播事件
///
/// 全局槽位与事件是互补的两条通道：事件驱动实时 UI，槽位负责在
/// 前端切换页面/标签导致监听重建、甚至页面重载后仍能恢复进度。
fn emit_progress(app: &AppHandle, progress: SingboxUpdateProgress) {
    if let Ok(mut slot) = progress_slot().write() {
        *slot = progress.clone();
    }
    if let Err(e) = app.emit(CORE_UPDATE_PROGRESS_EVENT, &progress) {
        log::warn!("[upgrade] 广播升级进度事件失败: {}", e);
    }
}

/// 推进到指定阶段并广播（保留已下载字节等累积字段）
fn set_stage(
    app: &AppHandle,
    stage: SingboxUpdateStage,
    percent: u8,
    message: &str,
) -> SingboxUpdateProgress {
    let mut p = read_progress();
    p.stage = stage;
    p.percent = percent;
    p.message = message.to_string();
    p.finished = stage.is_terminal();
    if stage == SingboxUpdateStage::Downloading {
        p.success = None;
        p.error = None;
    }
    emit_progress(app, p.clone());
    p
}

/// 字节数格式化为人类可读字符串（供进度文案使用）
fn format_bytes_human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    if bytes == 0 {
        return "0 B".to_string();
    }
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit])
    }
}

/// 查询内核升级当前进度
///
/// 前端在 App 启动与面板挂载时调用：后端是进度真值源，即使前端状态因
/// 切换页面/tab 或整页重载而丢失，也能据此复原按钮上的进度显示。
#[tauri::command]
pub fn core_upgrade_status() -> ApiResponse<SingboxUpdateProgress> {
    ApiResponse::ok(read_progress())
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingboxUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_notes: String,
    pub published_at: String,
    pub download_url: Option<String>,
    pub download_size: u64,
}

/// GitHub Release API 响应数据结构
#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GithubReleaseAsset>,
}

/// 获取当前已安装的 sing-box 版本号
pub fn get_current_singbox_version() -> String {
    if let Ok(path) = crate::core::sidecar::SidecarManager::resolve_binary_path() {
        if let Ok(output) = std::process::Command::new(path).arg("version").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.starts_with("sing-box version ") {
                    return line.replace("sing-box version ", "").trim().to_string();
                }
            }
            if let Some(first) = stdout.lines().next() {
                return first.trim().to_string();
            }
        }
    }
    "未知版本".to_string()
}

/// 检查 Sing-box GitHub 官方最新 Release
#[tauri::command]
pub async fn core_check_singbox_update() -> Result<ApiResponse<SingboxUpdateInfo>, AppError> {
    let current_ver = get_current_singbox_version();
    info!("当前 sing-box 版本: {}", current_ver);

    let client = reqwest::Client::builder()
        .user_agent("Auroweave-Client/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Network(format!("初始化 HTTP 客户端失败: {}", e)))?;

    let resp = client
        .get("https://api.github.com/repos/SagerNet/sing-box/releases/latest")
        .send()
        .await
        .map_err(|e| AppError::Network(format!("请求 GitHub Release 失败: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Network(format!("GitHub API 返回错误状态码: {}", resp.status())));
    }

    let release: GithubReleaseResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Network(format!("解析 Release 数据失败: {}", e)))?;

    let latest_ver = release.tag_name.trim_start_matches('v').to_string();
    let clean_current = current_ver.trim_start_matches('v').to_string();

    let has_update = !clean_current.is_empty()
        && clean_current != "未知版本"
        && clean_current != latest_ver;

    // 匹配当前平台架构的资产名称
    let target_keyword = get_platform_asset_keyword();
    let mut matched_url = None;
    let mut matched_size = 0;

    for asset in release.assets {
        if asset.name.contains(&target_keyword) {
            matched_url = Some(asset.browser_download_url);
            matched_size = asset.size;
            break;
        }
    }

    let info = SingboxUpdateInfo {
        current_version: current_ver,
        latest_version: release.tag_name,
        has_update,
        release_notes: release.body.unwrap_or_default(),
        published_at: release.published_at.unwrap_or_default(),
        download_url: matched_url,
        download_size: matched_size,
    };

    Ok(ApiResponse::ok(info))
}

fn get_platform_asset_keyword() -> String {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "darwin-arm64.tar.gz".to_string();

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "darwin-amd64.tar.gz".to_string();

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "windows-amd64.zip".to_string();

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "linux-amd64.tar.gz".to_string();

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "linux-arm64.tar.gz".to_string();

    #[allow(unreachable_code)]
    "tar.gz".to_string()
}

/// 内核升级包下载来源白名单（域名后缀匹配）
///
/// 只允许 GitHub 官方 Release 域名，防止任意 URL 被传入下载二进制并替换内核
/// （内核二进制以 SUID/管理员权限运行，被替换意味着任意代码提权执行）。
const ALLOWED_DOWNLOAD_HOSTS: [&str; 4] = [
    "github.com",
    "objects.githubusercontent.com",
    "github-releases.githubusercontent.com",
    "api.github.com",
];

/// 校验下载 URL 的 host 是否在白名单内（精确匹配或子域名后缀匹配）
fn validate_download_url(url_str: &str) -> Result<(), AppError> {
    let parsed = url::Url::parse(url_str)
        .map_err(|e| AppError::Validation(format!("无效的下载链接: {}", e)))?;
    // 仅允许 HTTPS（防中间人替换二进制）
    if parsed.scheme() != "https" {
        return Err(AppError::Validation("下载链接必须使用 HTTPS".to_string()));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::Validation("下载链接缺少主机名".to_string()))?;
    let is_allowed = ALLOWED_DOWNLOAD_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{}", allowed)));
    if is_allowed {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "不允许的下载来源: {}（仅支持 GitHub 官方域名）",
            host
        )))
    }
}

/// 计算 SHA-256（十六进制小写）
fn compute_sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// 执行内核在线下载、解压并替换升级
///
/// 安全与稳定性约束：
/// 1. 下载来源域名白名单（仅 GitHub 官方 Release 域）+ 强制 HTTPS
/// 2. 可选 sha256 参数：提供时下载后校验哈希，不匹配则中止（不会替换现有内核）
/// 3. 升级前若内核正在运行，先停止内核再复制，避免目标文件被占用导致升级必然失败
/// 4. 临时目录带进程唯一后缀（auroweave_update_{uuid}），避免并发实例互相干扰
/// 5. 互斥：同一时刻只允许一个升级任务，避免并发下载/替换互相破坏内核文件
///
/// 进度语义：全程通过 `core-update-progress` 事件 + 全局进度槽位双通道上报，
/// 前端按钮据此渲染百分比与阶段文案。切换页面/tab 或整页重载后，
/// 前端可调用 `core_upgrade_status` 回查后端真值复原进度。
/// 抢占升级闸门并写入首帧进度
///
/// 检查与占位必须在同一个写锁临界区内完成：若拆成"先读后判、再置位"，
/// 用户双击"立即在线升级"时两次 invoke 可能都读到空闲态，从而并发下载
/// 并同时替换内核文件。
fn try_begin_upgrade(app: &AppHandle) -> Result<(), AppError> {
    let snapshot = {
        let mut slot = progress_slot()
            .write()
            .map_err(|_| AppError::Unknown("内核升级状态锁已损坏".to_string()))?;
        if !slot.finished && slot.stage != SingboxUpdateStage::Idle {
            return Err(AppError::Validation(
                "内核升级正在进行中，请等待当前任务完成".to_string(),
            ));
        }
        *slot = SingboxUpdateProgress {
            stage: SingboxUpdateStage::Preparing,
            percent: 0,
            message: "正在准备下载内核安装包...".to_string(),
            downloaded: 0,
            total: 0,
            finished: false,
            success: None,
            error: None,
        };
        slot.clone()
    };
    let _ = app.emit(CORE_UPDATE_PROGRESS_EVENT, &snapshot);
    Ok(())
}

/// 执行内核在线下载、解压并替换升级
///
/// 安全与稳定性约束：
/// 1. 下载来源域名白名单（仅 GitHub 官方 Release 域）+ 强制 HTTPS
/// 2. 可选 sha256 参数：提供时下载后校验哈希，不匹配则中止（不会替换现有内核）
/// 3. 升级前若内核正在运行，先停止内核再复制，避免目标文件被占用导致升级必然失败
/// 4. 临时目录带进程唯一后缀（auroweave_update_{uuid}），避免并发实例互相干扰
/// 5. 互斥：同一时刻只允许一个升级任务，避免并发下载/替换互相破坏内核文件
///
/// 本命令是 **fire-and-forget**（同 speedtest_run_batch）：同步做完白名单校验
/// 与闸门抢占后立即返回，真正的长耗时流程在后台任务里跑，进度一律走
/// `core-update-progress` 事件。这样前端无需挂一个数分钟的 IPC Promise ——
/// 用户切页/切 tab 甚至整页重载都不会让升级被取消或让前端状态与后端脱节。
#[tauri::command]
pub async fn core_upgrade_singbox(
    app_handle: AppHandle,
    download_url: String,
    sha256: Option<String>,
) -> Result<ApiResponse<()>, AppError> {
    // 0. 下载来源白名单校验（先于任何网络请求，且需同步回错给调用方）
    validate_download_url(&download_url)?;

    // 0.1 抢占闸门：已有升级任务在途时直接拒绝
    try_begin_upgrade(&app_handle)?;

    info!("开始下载 sing-box 内核升级包: {}", download_url);

    let app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        // 任一阶段失败都必须落到 failed 终态，否则前端按钮会永久停在
        // "正在替换内核文件..."，用户既看不到失败原因也无法重试
        if let Err(e) = run_upgrade(&app, &download_url, sha256.as_deref()).await {
            let msg = e.to_string();
            let mut p = read_progress();
            p.stage = SingboxUpdateStage::Failed;
            p.message = format!("内核升级失败: {}", msg);
            p.finished = true;
            p.success = Some(false);
            p.error = Some(msg);
            emit_progress(&app, p);
        }
    });

    Ok(ApiResponse::ok(()))
}

/// 流式下载升级包并按节流间隔回调真实进度
///
/// 抽成独立函数有两个原因：
/// 1. 生产路径只关心"把进度 emit 出去"，不必关心 HTTP 分块细节；
/// 2. 进度节流/百分比映射是纯逻辑，抽出后可用真实网络做端到端验证
///    （见 tests 中的 e2e 用例），而不必拉起整个 Tauri 运行时。
///
/// 回调参数：`(已下载字节, 总字节, 百分比, 展示文案)`。
/// 总字节为 0 表示服务端未返回 Content-Length（chunked 传输），
/// 此时百分比恒为 0，由调用方决定降级展示策略。
async fn download_with_progress<F>(
    client: reqwest::Client,
    url: &str,
    mut on_progress: F,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(u64, u64, u8, String),
{
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("下载内核升级包失败: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Network(format!("下载返回异常状态: {}", resp.status())));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut bytes: Vec<u8> = Vec::with_capacity(total as usize);
    let mut last_emit = Instant::now();

    // 首帧：让按钮立刻从"准备中"切到"下载中"并显示 0 起点
    on_progress(
        0,
        total,
        0,
        if total > 0 {
            format!("正在下载内核安装包 (0 B/{})", format_bytes_human(total))
        } else {
            "正在下载内核安装包 (0 B)".to_string()
        },
    );

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| AppError::Network(format!("读取升级包数据流失败: {}", e)))?
    {
        bytes.extend_from_slice(&chunk);

        // 节流：每 DOWNLOAD_EMIT_INTERVAL 或下载收尾时报一次，
        // 避免每 KB 一次事件挤爆 IPC 通道
        let is_last = total > 0 && bytes.len() as u64 >= total;
        if !is_last && last_emit.elapsed() < DOWNLOAD_EMIT_INTERVAL {
            continue;
        }
        last_emit = Instant::now();

        let downloaded = bytes.len() as u64;
        // 下载段映射到整体进度的前 70%；total 未知时退化为不确定态
        let percent = if total > 0 {
            ((downloaded.min(total) * DOWNLOAD_PERCENT_CAP) / total) as u8
        } else {
            0
        };
        let message = if total > 0 {
            format!(
                "正在下载内核安装包 ({}/{})",
                format_bytes_human(downloaded),
                format_bytes_human(total)
            )
        } else {
            format!("正在下载内核安装包 ({})", format_bytes_human(downloaded))
        };
        on_progress(downloaded, total, percent, message);
    }

    Ok(bytes)
}

/// 从已下载的升级包二进制中解析版本号（如 1.14.2）
///
/// 优先用 `sing-box version` 自述，失败则回退到解压根目录名
/// （官方包固定为 `sing-box-<ver>-<os>-<arch>/`）。两者都拿不到时
/// 返回 None，由调用方决定降级策略——绝不猜一个版本号写进文件名。
fn detect_new_version(source_bin: &std::path::Path, extract_dir: &std::path::Path) -> Option<String> {
    // 1) 自述版本最可靠
    if let Ok(out) = std::process::Command::new(source_bin).arg("version").output() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if let Some(rest) = line.trim().strip_prefix("sing-box version ") {
                let v = rest.trim().trim_start_matches('v');
                if crate::core_paths::is_valid_version(v) {
                    return Some(v.to_string());
                }
            }
        }
    }
    // 2) 回退：解压根目录名提取版本段。
    //    注意必须只认"子目录"：tar 解压后根目录下同时躺着二进制文件本身
    //    （sing-box）与版本目录（sing-box-1.14.2-darwin-arm64），
    //    read_dir 顺序不保证，取到文件就会解析失败。
    let top_dir = std::fs::read_dir(extract_dir)
        .ok()
        .and_then(|entries| {
            entries
                .flatten()
                .find(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().to_string())
        });
    if let Some(top) = top_dir {
        // 官方包目录名形如 sing-box-1.14.2-darwin-arm64。
        // 注意不能简单 split('-') 后取 parts[1]："sing-box" 本身含连字符，
        // 切出来是 ["sing","box","1.14.2",...]，按下标取会永远取错。
        // 正确做法：先剥掉 "sing-box-" 前缀，剩下的 "1.14.2-darwin-arm64"
        // 再按首个连字符切出版本段。
        if let Some(rest) = top.strip_prefix("sing-box-") {
            let ver = rest.split('-').next().unwrap_or("");
            if crate::core_paths::is_valid_version(ver) {
                return Some(ver.to_string());
            }
        }
    }
    None
}

/// service 模式下的内核升级：暂存 → 停服务 → UAC 提权应用 → 重启
///
/// 仅 Windows。调用方已确认 `core.run_mode == "service"`。
///
/// 流程与普通模式的差别只有"谁能写 bin/"：
/// - 普通模式：GUI 自己 `fs::copy`；
/// - service 模式：bin/ 的 ACL 禁止普通用户写入，必须由以 SYSTEM 运行的
///   AuroDaemon 落盘。GUI 负责把新内核放进用户可写的暂存目录，然后通过
///   ShellExecuteExW + `runas` 弹一次 UAC，由 `AuroDaemon apply-core`
///   完成"校验暂存路径白名单 → 复制 → 复验哈希 → 回收旧版本"。
///
/// 为什么先停服务：内核进程运行时其可执行文件被锁，复制必然失败。
/// 这里用 IPC `SHUTDOWN_CORE`（服务已有该指令），而不是 kill 子进程——
/// 后者对服务托管的进程无效。
#[cfg(target_os = "windows")]
async fn upgrade_via_service(
    app_handle: &AppHandle,
    source_bin: &std::path::Path,
    new_version: &str,
) -> Result<(), AppError> {
    // ---- 步骤1: 放入暂存目录（用户可写）----
    let staging_dir = crate::get_data_root().join("update_staging");
    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| AppError::Io(format!("创建内核暂存目录失败: {}", e)))?;
    let staged = staging_dir.join(crate::core_paths::core_file_name(new_version));
    std::fs::copy(source_bin, &staged)
        .map_err(|e| AppError::Io(format!("写入内核暂存区失败: {}", e)))?;
    info!("[upgrade] 新内核已暂存: {:?}", staged);

    // ---- 步骤2: 通过 IPC 停掉服务托管的内核 ----
    set_stage(
        app_handle,
        SingboxUpdateStage::StoppingCore,
        92,
        "正在停止内核服务...",
    );
    match crate::core::ipc_client::send_ipc_request("SHUTDOWN_CORE", None).await {
        Ok(resp) if resp.success => {
            info!("[upgrade] 服务已停止内核");
        }
        Ok(resp) => {
            log::warn!("[upgrade] 停止内核返回失败: {}", resp.error.unwrap_or_default());
        }
        Err(e) => {
            // 服务未运行也可能是 Err，这里仅记录不阻断：
            // 最终能否复制由 apply-core 的错误信息给出，比在这里猜更准
            log::warn!("[upgrade] 停止内核 IPC 失败（服务可能本就未运行）: {}", e);
        }
    }
    // 等待文件句柄释放
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // ---- 步骤3: UAC 提权应用 ----
    set_stage(
        app_handle,
        SingboxUpdateStage::Replacing,
        95,
        "请在弹出的授权窗口中确认安装内核...",
    );
    let action = format!("apply-core --staged \"{}\"", staged.display());
    if let Err(e) = crate::system::service_control::execute_uac_action(app_handle, &action) {
        let _ = std::fs::remove_file(&staged);
        return Err(AppError::Permission(format!(
            "内核安装失败（需要管理员权限）: {}",
            e
        )));
    }
    let _ = std::fs::remove_file(&staged);

    // ---- 步骤4: 重启内核 ----
    set_stage(
        app_handle,
        SingboxUpdateStage::Restarting,
        98,
        "正在重新拉起内核...",
    );
    let _ = crate::system::startup::apply_core_mode_with_fallback(app_handle).await;

    let mut done = read_progress();
    done.stage = SingboxUpdateStage::Done;
    done.percent = 100;
    done.message = format!("内核已升级到 {}，并已重新拉起运行", new_version);
    done.finished = true;
    done.success = Some(true);
    done.error = None;
    emit_progress(app_handle, done);
    Ok(())
}

/// 升级主流程（由 `core_upgrade_singbox` 在后台任务中调用，负责上报成功终态）
async fn run_upgrade(
    app_handle: &AppHandle,
    download_url: &str,
    sha256: Option<&str>,
) -> Result<(), AppError> {
    let app_handle = app_handle.clone();
    let download_url = download_url.to_string();

    let client = reqwest::Client::builder()
        .user_agent("Auroweave-Client/1.0")
        // 大版本内核包体积可达数十 MB，慢速网络下 120s 不够；
        // 600s 兼顾慢速网络与异常卡死（连接级别失败由系统 TCP 超时兜底）
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| AppError::Network(format!("初始化下载客户端失败: {}", e)))?;

    // 流式读取：逐块累加并按节流间隔上报真实下载进度。
    // 早期实现用 resp.bytes().await 一次性收全量，几十 MB 的包会让用户
    // 在按钮上盯着"请勿关闭"干等数分钟而毫无反馈。
    let bytes = download_with_progress(client, &download_url, |downloaded, total, percent, message| {
        let mut p = read_progress();
        p.stage = SingboxUpdateStage::Downloading;
        p.percent = percent;
        p.message = message;
        p.downloaded = downloaded;
        p.total = total;
        p.finished = false;
        emit_progress(&app_handle, p);
    })
    .await?;

    // 1. 可选 SHA-256 校验：不匹配直接中止，不影响现有内核
    if let Some(expected) = sha256 {
        set_stage(
            &app_handle,
            SingboxUpdateStage::Verifying,
            72,
            "正在校验升级包完整性...",
        );
        let expected = expected.trim().to_lowercase();
        let actual = compute_sha256_hex(&bytes);
        if actual != expected {
            return Err(AppError::Validation(format!(
                "升级包 SHA-256 校验失败: 期望 {}，实际 {}",
                expected, actual
            )));
        }
        info!("升级包 SHA-256 校验通过: {}", actual);
    }

    // 2. 进程唯一临时目录，避免并发升级实例共用目录互相覆盖
    set_stage(
        &app_handle,
        SingboxUpdateStage::Extracting,
        78,
        "正在解压内核安装包...",
    );
    let temp_dir = std::env::temp_dir()
        .join(format!("auroweave_update_{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| AppError::Io(format!("创建临时目录失败: {}", e)))?;

    let archive_path = if download_url.ends_with(".zip") {
        temp_dir.join("singbox_update.zip")
    } else {
        temp_dir.join("singbox_update.tar.gz")
    };

    std::fs::write(&archive_path, &bytes)
        .map_err(|e| AppError::Io(format!("保存临时升级包失败: {}", e)))?;

    info!("升级包已写入临时文件: {:?}，开始解压", archive_path);

    // 解压到 temp_dir/extracted
    let extract_dir = temp_dir.join("extracted");
    let _ = std::fs::remove_dir_all(&extract_dir);
    let _ = std::fs::create_dir_all(&extract_dir);

    #[cfg(not(target_os = "windows"))]
    {
        let status = std::process::Command::new("tar")
            .arg("-xzf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&extract_dir)
            .status()
            .map_err(|e| AppError::Io(format!("解压 tar.gz 失败: {}", e)))?;

        if !status.success() {
            return Err(AppError::Sidecar("tar 解压升级包返回错误退出码".to_string()));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("tar")
            .arg("-xf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&extract_dir)
            .status()
            .map_err(|e| AppError::Io(format!("解压 zip 失败: {}", e)))?;

        if !status.success() {
            return Err(AppError::Sidecar("tar 解压 zip 升级包返回错误退出码".to_string()));
        }
    }

    // 在 extract_dir 中递归寻找 sing-box 二进制文件
    let mut found_bin: Option<PathBuf> = None;
    for entry in walkdir(&extract_dir) {
        let name = entry.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        #[cfg(target_os = "windows")]
        if name == "sing-box.exe" {
            found_bin = Some(entry);
            break;
        }
        #[cfg(not(target_os = "windows"))]
        if name == "sing-box" {
            found_bin = Some(entry);
            break;
        }
    }

    let source_bin = found_bin.ok_or_else(|| AppError::Sidecar("解压包内未找到 sing-box 可执行文件".to_string()))?;
    info!("找到新版本 sing-box 二进制文件: {:?}", source_bin);

    // 解析新版本号：目标文件名要带版本号（见下方 upgrade_bin_dir 说明）
    let new_version = detect_new_version(&source_bin, &extract_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&temp_dir);
        AppError::Sidecar(
            "无法从升级包中识别内核版本号，为避免覆盖出错已中止升级".to_string(),
        )
    })?;
    info!("[upgrade] 新内核版本: {}", new_version);

    // ------------------------------------------------------------------
    // service 模式（Windows）：走暂存 + UAC 提权应用，不能直接写 bin/
    //
    // 原因：安装服务时 installer 用 SDDL 把 bin/ 锁成
    // `Authenticated Users: FRGX`（只读+执行），这是刻意的安全设计，
    // 用于防止普通用户替换以 SYSTEM 权限运行的提权组件。因此 GUI 进程
    // 对 bin/ **没有写权限**——此前直接 fs::copy 必然失败。
    //
    // 正确路径：把新内核放到用户可写的暂存目录 → 经 UAC 拉起
    // AuroDaemon apply-core（以管理员/SYSTEM 权限）完成复制与校验。
    //
    // 注意：此处不要预先 set_stage(Replacing)。upgrade_via_service 内部
    // 会按"暂存 → 停内核 → 提权应用 → 重启"自行推进阶段；若在此先报
    // Replacing(90)，紧跟其后的 StoppingCore(92) 之前的一次进度会上报
    // 出更低的阶段，表现为进度条在 90% 处倒退。
    // ------------------------------------------------------------------
    #[cfg(target_os = "windows")]
    {
        let settings = crate::commands::settings::settings_get_internal(&app_handle);
        if settings.core.run_mode == "service" {
            return upgrade_via_service(&app_handle, &source_bin, &new_version).await;
        }
    }

    // SUID 恢复失败原因（成功则为 None），用于终态文案告知用户 TUN 已退化
    let mut suid_failed_reason: Option<String> = None;

    // 目标路径由 core_paths 统一决定（版本化命名 + 唯一目录），
    // 这里只负责"是哪一版"这一升级特有的信息。
    let bin_dir = crate::core_paths::bin_dir();
    let _ = std::fs::create_dir_all(&bin_dir);
    let target_bin = bin_dir.join(crate::core_paths::core_file_name(&new_version));

    // 升级前的旧内核：用于继承 SUID 属性 + 升级成功后清理
    let old_bin = crate::core_paths::resolve_core_binary();
    // 仅当旧内核确实位于同一个 bin 目录时才视为"将被替换的旧版本"，
    // 避免把开发目录（sidecar-bin）里的内核误删
    let old_is_in_bin_dir = old_bin
        .as_ref()
        .map(|p| p.parent() == Some(bin_dir.as_path()))
        .unwrap_or(false);
    let old_needed_suid = old_bin
        .as_ref()
        .map(|p| crate::core::sidecar::SidecarManager::is_privileged_binary(p))
        .unwrap_or(false);
    if let Some(ref ob) = old_bin {
        info!(
            "[upgrade] 现有内核: {:?} (同目录={}, 需SUID={})",
            ob, old_is_in_bin_dir, old_needed_suid
        );
    }

    // 升级前先停止运行中的内核：否则复制目标被占用必然失败
    // （Windows 上目标文件被运行中进程锁定；Unix 上即使 rename 成功，运行中的也是旧 inode）
    {
        set_stage(
            &app_handle,
            SingboxUpdateStage::StoppingCore,
            84,
            "正在停止运行中的内核...",
        );
        let running_res = crate::commands::settings::core_query_running(app_handle.clone()).await;
        let is_running = running_res.data.unwrap_or(false);
        if is_running {
            info!("[upgrade] 检测到内核正在运行，先停止内核再替换二进制...");
            let sm = app_handle
                .state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>()
                .inner()
                .clone();
            if let Err(e) = sm.stop().await {
                log::warn!("[upgrade] 停止旧内核失败（继续尝试复制，若被占用将返回明确错误）: {}", e);
            }
            // 给操作系统一点时间释放文件句柄
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
    }

    // 替换目标文件：失败时给出明确指引并清理临时目录
    set_stage(
        &app_handle,
        SingboxUpdateStage::Replacing,
        90,
        "正在替换内核文件...",
    );
    info!("将新内核复制到目标路径: {:?}", target_bin);
    if let Err(e) = std::fs::copy(&source_bin, &target_bin) {
        // 复制失败最常见原因是目标仍被运行中的内核/服务占用
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(AppError::Io(format!(
            "复制内核文件失败: {}。请先在设置中停止内核/系统服务后再升级",
            e
        )));
    }


    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&target_bin) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&target_bin, perms);
        }
    }

    // macOS TUN 模式依赖 SUID root。fs::copy 以新建文件方式写入，SUID 位必然丢失；
    // 早期实现只打一条 warn 日志就结束，用户升级后 TUN 直接不可用且无任何提示。
    // 现在：只要旧内核本来就有 SUID，就立即向用户申请一次管理员授权把新内核提权，
    // 保证"升级前后内核具备同等权限"，不把问题留到下次启动。
    #[cfg(target_os = "macos")]
    let suid_restored: bool = {
        if old_needed_suid {
            set_stage(
                &app_handle,
                SingboxUpdateStage::Replacing,
                93,
                "正在恢复内核管理员权限（macOS TUN 模式需要）...",
            );
            match crate::core::sidecar::SidecarManager::ensure_privileged_binary(&target_bin).await {
                Ok(()) => {
                    info!("[upgrade] 新内核已恢复 SUID root 权限: {:?}", target_bin);
                    true
                }
                Err(e) => {
                    // 提权失败不阻断升级：新内核本身可用，只是 TUN 模式会退化。
                    // 必须让用户知道，否则表现为"TUN 莫名其妙坏了"
                    log::warn!("[upgrade] 恢复 SUID 权限失败，TUN 模式将退化: {}", e);
                    suid_failed_reason = Some(e.to_string());
                    false
                }
            }
        } else {
            // 旧内核本就没有 SUID（非 TUN 用户），保持现状即可，不平白弹授权框
            false
        }
    };
    #[cfg(not(target_os = "macos"))]
    let suid_restored: bool = true;

    // 清理临时文件
    let _ = std::fs::remove_dir_all(&temp_dir);

    // 触发内核平滑重启
    set_stage(
        &app_handle,
        SingboxUpdateStage::Restarting,
        96,
        "正在重新拉起内核...",
    );
    let _ = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await;
    info!("sing-box 内核升级完成并已重新拉起运行");

    // 回收历史内核文件：放在重启成功之后，确保新内核确实可用时才删旧的。
    // 顺序很重要——先删后启，一旦新内核起不来就再没有回退版本了。
    if old_is_in_bin_dir {
        crate::core_paths::cleanup_stale_core_binaries(&bin_dir, &target_bin);
    } else {
        log::info!(
            "[upgrade] 现有内核不在 {} 中，跳过历史清理（开发环境 sidecar-bin 不受影响）",
            bin_dir.display()
        );
    }

    // 终态：置 finished / success，供前端按钮与状态回查识别任务已结束
    let mut done = read_progress();
    done.stage = SingboxUpdateStage::Done;
    done.percent = 100;
    done.finished = true;
    done.success = Some(true);
    done.error = None;
    // SUID 恢复失败必须如实反映到终态文案里：内核已升级成功，
    // 但 TUN 模式会退化，用户需要知道而不是事后自己排查
    done.message = match &suid_failed_reason {
        Some(_) => format!(
            "内核已升级到 {}，但未能恢复管理员权限，TUN 模式暂不可用（可在 TUN 面板重新授权）",
            new_version
        ),
        None if suid_restored => {
            format!("内核已升级到 {}，并已重新拉起运行", new_version)
        }
        None => format!("内核已升级到 {}，已重新拉起运行", new_version),
    };
    if let Some(reason) = &suid_failed_reason {
        log::warn!("[upgrade] 终态提示: {}", reason);
    }
    emit_progress(&app_handle, done);

    Ok(())
}

/// 递归收集目录下的所有文件（用于在解压产物中定位内核二进制）
fn walkdir(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 下载段百分比：已下载/总量 映射到整体进度的前 DOWNLOAD_PERCENT_CAP
    fn download_percent(downloaded: u64, total: u64) -> u8 {
        if total == 0 {
            return 0;
        }
        ((downloaded.min(total) * DOWNLOAD_PERCENT_CAP) / total) as u8
    }

    #[test]
    fn download_percent_maps_into_download_weight() {
        assert_eq!(download_percent(0, 1000), 0);
        assert_eq!(download_percent(500, 1000), 35);
        assert_eq!(download_percent(1000, 1000), DOWNLOAD_PERCENT_CAP as u8);
    }

    #[test]
    fn download_percent_clamps_when_more_bytes_than_content_length() {
        // CDN 改写 Content-Length 时已下载可能超过 total，
        // 绝不能让百分比越过 70 侵占后续阶段（否则进度条倒退）
        assert_eq!(download_percent(1500, 1000), DOWNLOAD_PERCENT_CAP as u8);
    }

    #[test]
    fn download_percent_is_zero_when_total_unknown() {
        // chunked 传输无 Content-Length：退化为不确定态而非除零 panic
        assert_eq!(download_percent(5000, 0), 0);
    }

    #[test]
    fn download_percent_never_decreases_monotonic() {
        let total = 3_400_000u64;
        let mut last = 0u8;
        for downloaded in (0..=total).step_by(50_000) {
            let pct = download_percent(downloaded, total);
            assert!(pct >= last, "进度回退: {downloaded} -> {pct} (上次 {last})");
            last = pct;
        }
        assert_eq!(last, DOWNLOAD_PERCENT_CAP as u8);
    }

    #[test]
    fn stage_anchors_ascend_after_download() {
        // 关键不变量：下载(≤70) < 校验(72) < 解压(78) < 停核(84)
        // < 替换(90) < 重启(96) < 完成(100)
        let anchors = [
            (SingboxUpdateStage::Downloading, DOWNLOAD_PERCENT_CAP as u8),
            (SingboxUpdateStage::Verifying, 72),
            (SingboxUpdateStage::Extracting, 78),
            (SingboxUpdateStage::StoppingCore, 84),
            (SingboxUpdateStage::Replacing, 90),
            (SingboxUpdateStage::Restarting, 96),
            (SingboxUpdateStage::Done, 100),
        ];
        for w in anchors.windows(2) {
            assert!(w[0].1 < w[1].1, "阶段锚点倒挂: {:?} 之后 {:?}", w[0].0, w[1].0);
        }
    }

    #[test]
    fn only_done_and_failed_are_terminal() {
        // 前端据 finished 解锁按钮，判定错误会永久锁死或永不显示结果
        assert!(SingboxUpdateStage::Done.is_terminal());
        assert!(SingboxUpdateStage::Failed.is_terminal());
        for stage in [
            SingboxUpdateStage::Idle,
            SingboxUpdateStage::Preparing,
            SingboxUpdateStage::Downloading,
            SingboxUpdateStage::Verifying,
            SingboxUpdateStage::Extracting,
            SingboxUpdateStage::StoppingCore,
            SingboxUpdateStage::Replacing,
            SingboxUpdateStage::Restarting,
        ] {
            assert!(!stage.is_terminal(), "{:?} 不应判定为终态", stage);
        }
    }

    #[test]
    fn stage_serializes_to_snake_case_matching_ts_union() {
        // 前端 SingboxUpdateStage 联合类型逐字对齐这些字符串
        let cases = [
            (SingboxUpdateStage::Idle, "\"idle\""),
            (SingboxUpdateStage::Preparing, "\"preparing\""),
            (SingboxUpdateStage::Downloading, "\"downloading\""),
            (SingboxUpdateStage::Verifying, "\"verifying\""),
            (SingboxUpdateStage::Extracting, "\"extracting\""),
            (SingboxUpdateStage::StoppingCore, "\"stopping_core\""),
            (SingboxUpdateStage::Replacing, "\"replacing\""),
            (SingboxUpdateStage::Restarting, "\"restarting\""),
            (SingboxUpdateStage::Done, "\"done\""),
            (SingboxUpdateStage::Failed, "\"failed\""),
        ];
        for (stage, expected) in cases {
            assert_eq!(serde_json::to_string(&stage).unwrap(), expected);
        }
    }

    #[test]
    fn idle_progress_is_not_upgrading() {
        // init() 依赖此判定决定是否回填：idle 必须算"未在途"
        let p = SingboxUpdateProgress::idle();
        assert_eq!(p.stage, SingboxUpdateStage::Idle);
        assert!(!p.finished);
        let is_upgrading = !p.finished && p.stage != SingboxUpdateStage::Idle;
        assert!(!is_upgrading, "idle 不得判定为升级中，否则会复活残留状态");
    }

    #[test]
    fn download_url_whitelist_accepts_official_github_hosts() {
        for url in [
            "https://github.com/SagerNet/sing-box/releases/download/v1.13.0/sing-box-linux-amd64.tar.gz",
            "https://objects.githubusercontent.com/github-production-release-asset/abc",
            "https://github-releases.githubusercontent.com/github-production-release-asset/abc",
        ] {
            assert!(validate_download_url(url).is_ok(), "应放行: {}", url);
        }
    }

    #[test]
    fn download_url_whitelist_rejects_non_github_and_plain_http() {
        // 内核以 SUID/管理员权限运行，被替换等于任意代码提权
        for url in [
            "https://evil.example.com/sing-box.tar.gz",
            "https://github.com.evil.com/x.tar.gz",
            "http://github.com/SagerNet/sing-box/releases/download/v1/sing-box.tar.gz",
            "ftp://github.com/x.tar.gz",
        ] {
            assert!(validate_download_url(url).is_err(), "应拒绝: {}", url);
        }
    }

    #[test]
    fn format_bytes_human_matches_expected_units() {
        assert_eq!(format_bytes_human(0), "0 B");
        assert_eq!(format_bytes_human(512), "512 B");
        assert_eq!(format_bytes_human(1024), "1.0 KB");
        assert_eq!(format_bytes_human(14_300_000), "13.6 MB");
    }

    #[test]
    fn detect_version_from_extracted_dir_name() {
        // 官方包解压根目录固定为 sing-box-<ver>-<os>-<arch>
        let dir = std::env::temp_dir().join(format!("auroweave_ver_{}", std::process::id()));
        let top = dir.join("sing-box-1.14.2-darwin-arm64");
        std::fs::create_dir_all(&top).unwrap();
        // 不可执行的占位文件 → version 自述必失败，走目录名回退
        let fake = top.join("sing-box");
        std::fs::write(&fake, b"not a real binary").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o644));
        }
        let v = detect_new_version(&fake, &dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(v, Some("1.14.2".to_string()));
    }

    #[test]
    fn detect_version_returns_none_when_unidentifiable() {
        // 识别不出必须返回 None 让调用方中止，绝不能猜版本写进文件名
        let dir = std::env::temp_dir().join(format!("auroweave_vnone_{}", std::process::id()));
        let top = dir.join("weird-name");
        std::fs::create_dir_all(&top).unwrap();
        let fake = top.join("sing-box");
        std::fs::write(&fake, b"junk").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o644));
        }
        let v = detect_new_version(&fake, &dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(v, None);
    }
    /// 真机联调（快）：对真实 GitHub 小资产验证流式进度链路。
    ///
    /// 默认跳过（`#[ignore]`）：需外网。开启：
    ///   cargo test --lib e2e_real_download -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "需外网，默认跳过"]
    async fn e2e_real_download_reports_monotonic_progress() {
        let url = "https://github.com/SagerNet/sing-box/releases/download/v1.14.2/SFA-version-metadata.json";
        let client = reqwest::Client::builder()
            .user_agent("Auroweave-Client/1.0")
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        let bytes = download_with_progress(client, url, move |d, t, p, m| {
            sink.lock().unwrap().push((d, t, p, m));
        })
        .await
        .expect("真实下载应成功");
        let events = seen.lock().unwrap().clone();
        println!("e2e: 下载 {} 字节, {} 次回调", bytes.len(), events.len());
        for (d, t, p, m) in events.iter() {
            println!("  downloaded={} total={} percent={} msg={}", d, t, p, m);
        }
        assert!(!events.is_empty(), "应至少有一次首帧回调");
        assert_eq!(events[0].0, 0, "首帧已下载应为 0");
        let total = events[0].1;
        assert!(total > 0, "GitHub 应返回 Content-Length，实际: {}", total);
        assert_eq!(bytes.len() as u64, total, "实收字节应与 Content-Length 一致");
        let last = events.last().unwrap();
        assert_eq!(last.0, total, "收尾回调应报告满字节");
        assert_eq!(last.2, DOWNLOAD_PERCENT_CAP as u8, "收尾百分比应等于下载段上限");
        let mut prev_bytes = 0u64;
        let mut prev_pct = 0u8;
        for (d, _, p, _) in events.iter() {
            assert!(*d >= prev_bytes, "字节数回退: {} -> {}", prev_bytes, d);
            assert!(*p >= prev_pct, "百分比回退: {} -> {}", prev_pct, p);
            prev_bytes = *d;
            prev_pct = *p;
        }
    }

    /// 真机联调（大资产 27MB）：验证节流在多 MB 传输中确实生效。
    ///
    /// 单独标注且默认跳过：当前网络到 release-assets 主机带宽很低
    /// （实测 10~400 KB/s 波动），整包可能耗时十分钟或因链路抖动中断。
    #[tokio::test]
    #[ignore = "需外网 + 27MB 真实下载，网络慢时可能超时，默认跳过"]
    async fn e2e_large_asset_throttles_progress_callbacks() {
        let url = "https://github.com/SagerNet/sing-box/releases/download/v1.14.2/sing-box-1.14.2-darwin-arm64.tar.gz";
        let client = reqwest::Client::builder()
            .user_agent("Auroweave-Client/1.0")
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .unwrap();
        let count = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let last_pct = std::sync::Arc::new(std::sync::Mutex::new(0u8));
        let c = count.clone();
        let lp = last_pct.clone();
        let res = download_with_progress(client, url, move |_d, _t, p, _m| {
            *c.lock().unwrap() += 1;
            let mut g = lp.lock().unwrap();
            assert!(p >= *g, "百分比回退: {} -> {}", *g, p);
            *g = p;
        })
        .await;
        let bytes = match res {
            Ok(b) => b,
            Err(e) => {
                println!("e2e-large: 下载中断（网络环境）: {}", e);
                println!(
                    "e2e-large: 中断前已回调 {} 次, 末态百分比 {}",
                    count.lock().unwrap(),
                    last_pct.lock().unwrap()
                );
                return;
            }
        };
        println!("e2e-large: 下载 {} 字节, {} 次回调", bytes.len(), count.lock().unwrap());
        let n = *count.lock().unwrap();
        assert!(
            n <= (bytes.len() / 1024).max(16),
            "节流失效：{} 字节产生 {} 次回调",
            bytes.len(),
            n
        );
    }
}
