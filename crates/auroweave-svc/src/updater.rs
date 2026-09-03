/// AuroDaemon 自更新模块
/// 作者: TanXiang
///
/// 在服务/计划任务启动时自动检测并应用二进制更新。
/// 主程序在触发计划任务前会生成 `manifest.json`，包含最新的 AuroDaemon.exe 和
/// sing-box.exe 的路径与 SHA-256 哈希。服务启动时比对哈希，发现差异则自动更新。
///
/// **信任模型（深度防御）**：
/// `manifest.json` 位于用户可写目录（`%ProgramData%\Auroweave\config`），
/// 其中的哈希由同一文件提供、可自证，因此 manifest 本身**不可信**。在尚未引入
/// 签名体系前的最大收紧策略：
/// 1. **源路径白名单**：`svc_path` / `singbox_path` 必须位于受信任目录
///    （`%ProgramData%\Auroweave\bin`、`%ProgramData%\Auroweave\update_staging`、
///    主程序安装目录）内，规范化后必须以白名单前缀开头，防止 manifest 指向
///    任意可执行文件
/// 2. **版本单调递增**：manifest 中的 `svc_version` 必须大于当前运行版本
///    （`env!("CARGO_PKG_VERSION")`）才允许执行 AuroDaemon 自替换，防降级攻击
/// 3. **复制后复验**：复制完成后对目标文件重新计算 SHA-256 并与 manifest 哈希
///    比对，防止复制中途（TOCTOU 窗口）源文件被替换
/// 4. **回滚**：自替换 spawn 失败时将 `.old` 改回原名，恢复旧版本继续运行
///
/// 更新流程：
/// 1. 清理上次更新遗留的 `AuroDaemon.old.exe`
/// 2. 读取 `manifest.json` 获取期望的文件路径、哈希与版本
/// 3. 校验源路径白名单；比对 sing-box.exe 哈希，不一致则覆盖更新并复验
/// 4. 比对 AuroDaemon.exe 自身哈希，满足版本单调递增且哈希不一致时执行自我更替：
///    a. 将自身重命名为 `AuroDaemon.old.exe`
///    b. 复制新版本到目标路径并复验哈希
///    c. 拉起新进程（传递相同参数）
///    d. 退出旧进程（Windows 服务模式下由新进程接管，本进程不再做额外清理；
///       计划任务模式下本进程不持有 core_manager，sing-box 由 Job Object
///       级联终止，详见 check_and_apply_updates 内注释）
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use tracing::{info, warn};

#[derive(serde::Deserialize)]
struct Manifest {
    svc_path: String,
    svc_hash: String,
    singbox_path: String,
    singbox_hash: String,
    /// manifest 生成方写入的 AuroDaemon 目标版本号（语义化版本字符串）。
    /// 缺省时视为版本检查不通过，拒绝自替换（防降级攻击的最小安全默认）。
    #[serde(default)]
    svc_version: Option<String>,
    /// 预留字段：manifest 的密码学签名。当前版本未实现签名校验，
    /// 字段先反序列化占位，待签名体系落地后启用。
    #[serde(default)]
    #[allow(dead_code)]
    signature: Option<String>,
}

/// 校验源路径是否位于受信任目录白名单内
///
/// 白名单（任一前缀匹配即可）：
/// - `%ProgramData%\Auroweave\bin`（安装/更新锁定目录，SYSTEM/Admin ACL）
/// - `%ProgramData%\Auroweave\update_staging`（更新暂存目录）
/// - 当前运行中的 AuroDaemon.exe 所在目录（主程序安装目录或锁定 bin 目录；
///   由已受信任的本进程路径推导，manifest 无法伪造）
///
/// 校验方式：`std::fs::canonicalize` 规范化后做前缀匹配，
/// 规范化会解析 `..`、符号链接与设备路径（`\\?\` 前缀），防止路径穿越。
fn is_trusted_source(src: &Path) -> bool {
    let canonical_src = match fs::canonicalize(src) {
        Ok(p) => p,
        Err(_) => return false, // 不存在的路径一律不信任
    };

    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let mut trusted_dirs: Vec<PathBuf> = vec![
        PathBuf::from(&program_data).join("Auroweave").join("bin"),
        PathBuf::from(&program_data).join("Auroweave").join("update_staging"),
    ];
    // 当前运行中的 exe 所在目录（主程序安装目录）
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            trusted_dirs.push(exe_dir.to_path_buf());
        }
    }

    for dir in &trusted_dirs {
        if let Ok(canonical_dir) = fs::canonicalize(dir) {
            if canonical_src.starts_with(&canonical_dir) {
                return true;
            }
        }
    }
    false
}

/// 语义化版本比较：a > b 则返回 true（仅比较 major.minor.patch）
///
/// 解析失败时返回 false（无法证明"更大"即拒绝更新，最小安全默认）。
fn version_gt(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Option<Vec<u64>> {
        let s = s.trim().trim_start_matches('v');
        let mut parts = Vec::new();
        for seg in s.split('.') {
            // 兼容 "0.1.0-beta" 之类的预发布后缀：只取数字段
            let num: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
            if num.is_empty() {
                return None;
            }
            parts.push(num.parse::<u64>().ok()?);
        }
        Some(parts)
    };
    let (Some(pa), Some(pb)) = (parse(a), parse(b)) else {
        return false;
    };
    pa > pb
}

/// 复制文件并在复制完成后复验目标文件 SHA-256 == 期望哈希
///
/// 复制完成到读取之间是 TOCTOU 窗口，源文件可能在复制中途被替换，
/// 因此复制后必须对目标文件重新哈希（而不是信任源文件此前算出的哈希）。
fn copy_and_verify_hash(src: &Path, dst: &Path, expect_hash: &str, name: &str) -> Result<(), String> {
    fs::copy(src, dst).map_err(|e| format!("复制 {} 失败: {}", name, e))?;
    let dst_hash = crate::utils::compute_sha256(dst)
        .ok_or_else(|| format!("复制后计算目标 {} 哈希失败", name))?;
    if dst_hash != expect_hash {
        // 复验失败：目标内容与 manifest 期望不符（复制窗口内被替换或 manifest 伪造）
        return Err(format!("{} 复制后哈希复验失败 ({} != {})", name, dst_hash, expect_hash));
    }
    Ok(())
}

/// 检查并应用二进制更新
///
/// 完整流程：
/// 1. 清理遗留的 `AuroDaemon.old.exe`（上次更新可能残留）
/// 2. 读取 `manifest.json`（不存在则跳过更新）
/// 3. 比对 sing-box.exe 哈希，不一致则覆盖更新（源路径须过白名单）并复验
/// 4. 比对 AuroDaemon.exe 自身哈希，版本单调递增且哈希不一致时执行自我更替
pub fn check_and_apply_updates() {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let config_dir = PathBuf::from(&program_data).join("Auroweave").join("config");
    let bin_dir = PathBuf::from(&program_data).join("Auroweave").join("bin");
    
    // ---- 步骤1: 清理上次更新遗留的 old.exe ----
    let old_svc_path = bin_dir.join("AuroDaemon.old.exe");
    if old_svc_path.exists() {
        if let Err(e) = fs::remove_file(&old_svc_path) {
            warn!("无法删除遗留的 old.exe: {}", e);
        } else {
            info!("成功清理遗留的 AuroDaemon.old.exe");
        }
    }
    
    // ---- 步骤2: 读取 manifest.json（不存在则跳过） ----
    let manifest_path = config_dir.join("manifest.json");
    if !manifest_path.exists() {
        return;
    }
    
    let content = match fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    
    let manifest: Manifest = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            warn!("解析 manifest.json 失败: {}", e);
            return;
        }
    };

    // manifest 位于用户可写目录，内容不可信：所有源路径必须过白名单校验
    let src_sb = PathBuf::from(&manifest.singbox_path);
    let src_svc = PathBuf::from(&manifest.svc_path);
    if !is_trusted_source(&src_sb) {
        warn!("manifest.singbox_path 不在受信任目录白名单内，拒绝更新: {}", manifest.singbox_path);
        return;
    }
    if !is_trusted_source(&src_svc) {
        warn!("manifest.svc_path 不在受信任目录白名单内，拒绝更新: {}", manifest.svc_path);
        return;
    }
    
    // ---- 步骤3: 检查 sing-box.exe 是否需要更新 ----
    // 比对当前文件的 SHA-256 与 manifest 中的期望哈希；
    // 哈希计算失败（None）时直接跳过该文件的更新，不做任何覆盖动作
    let target_sb_path = bin_dir.join("sing-box.exe");
    let current_sb_hash = crate::utils::compute_sha256(&target_sb_path);
    let sb_needs_update = match &current_sb_hash {
        None => {
            warn!("计算当前 sing-box.exe 哈希失败，跳过 sing-box 更新");
            false
        }
        Some(h) => h != &manifest.singbox_hash,
    };
    if sb_needs_update {
        if let Some(h) = &current_sb_hash {
            info!("发现 sing-box.exe 更新 ({} != {})，正在覆盖...", h, manifest.singbox_hash);
        }
        if src_sb.exists() {
            if let Err(e) = copy_and_verify_hash(&src_sb, &target_sb_path, &manifest.singbox_hash, "sing-box.exe") {
                warn!("sing-box.exe 更新失败: {}", e);
            } else {
                info!("sing-box.exe 更新完成，哈希复验通过");
            }
        }
    }
    
    // ---- 步骤4: 检查 AuroDaemon.exe 自身是否需要更新 ----
    // 双重门禁：哈希不一致 + manifest 版本号严格大于当前运行版本（防降级攻击）。
    // svc_version 缺省时直接拒绝自替换。
    let target_svc_path = std::env::current_exe().unwrap_or_else(|_| bin_dir.join("AuroDaemon.exe"));
    let current_svc_hash = crate::utils::compute_sha256(&target_svc_path);
    let svc_needs_update = match &current_svc_hash {
        None => {
            warn!("计算当前 AuroDaemon.exe 哈希失败，跳过自更新");
            false
        }
        Some(h) => h != &manifest.svc_hash,
    };
    if !svc_needs_update {
        return;
    }

    let Some(svc_version) = manifest.svc_version.as_deref() else {
        warn!("manifest 缺少 svc_version 字段，为防止降级攻击拒绝自更新");
        return;
    };
    if !version_gt(svc_version, env!("CARGO_PKG_VERSION")) {
        warn!(
            "manifest svc_version ({}) 不大于当前运行版本 ({})，为防止降级攻击拒绝自更新",
            svc_version, env!("CARGO_PKG_VERSION")
        );
        return;
    }

    info!("发现 AuroDaemon.exe 更新 (manifest 版本 {})，准备自我更替...", svc_version);
    if !src_svc.exists() {
        warn!("manifest.svc_path 指向的源文件不存在，跳过本次更新");
        return;
    }

    // 步骤4a: 重命名自身为 old.exe（正在运行的 exe 文件无法直接覆盖）
    if let Err(e) = fs::rename(&target_svc_path, &old_svc_path) {
        warn!("重命名自身失败，跳过本次更新: {}", e);
        return;
    }
    
    // 步骤4b: 复制新版本到目标路径，并在复制完成后复验哈希
    if let Err(e) = copy_and_verify_hash(&src_svc, &target_svc_path, &manifest.svc_hash, "AuroDaemon.exe") {
        warn!("{}", e);
        // 复制或复验失败：回滚，恢复旧版本继续运行
        let _ = fs::rename(&old_svc_path, &target_svc_path);
        return;
    }
    
    // 步骤4c: 拉起新进程并退出旧进程
    // 传递相同的命令行参数，确保新进程以相同模式运行
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cmd = Command::new(&target_svc_path);
    cmd.args(&args);
    
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    
    if let Err(e) = cmd.spawn() {
        warn!("拉起新进程失败: {}", e);
        // 回滚：把 .old rename 回原路径，恢复旧版本继续运行
        let _ = fs::rename(&old_svc_path, &target_svc_path);
        return;
    }
    
    // 成功拉起新进程。本函数在 main 的 tokio 运行时 block_on 内被调用，
    // 此处无法拿到 core_manager 的句柄做优雅停止（core_manager 在调用点之后才创建），
    // 由新进程负责清理：sing-box 与旧 AuroDaemon 绑定在同一 Job Object，
    // 旧进程 exit 后 OS 关闭 job 句柄触发 KILL_ON_JOB_CLOSE 级联终止 sing-box，
    // 新进程随后按相同配置重新拉起内核。
    info!("AuroDaemon.exe 自我更替成功，拉起新进程 (版本 {}) 并退出旧进程", svc_version);
    std::process::exit(0);
}
