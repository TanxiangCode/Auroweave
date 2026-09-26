/// Sing-box 二进制的唯一权威路径模块
///
/// 背景：历史上"内核放在哪、叫什么名字"散落在 5 处独立实现
/// （sidecar 暂存、GUI 在线升级、服务安装、服务自更新、服务侧查找），
/// 各自拼 `.../bin` 并各写各的文件名，导致：
///   - GUI 升级写 `sing-box-1.14.3`，服务 updater 却只认 `sing-box.exe`，
///     manifest 自更新会把旧版本盖回去，与 GUI 升级互相打架；
///   - 每次升级新建文件而不回收旧文件，bin/ 单调膨胀（实测 84MB→160MB）；
///   - service 模式（Windows）内核由 AuroDaemon 托管，GUI 却按本地子进程
///     的方式去 kill，压根停不掉，替换必然因文件占用而失败。
///
/// 本模块把"目录"与"文件名"两个决策收敛为唯一真源，其余代码一律派生使用。
/// 命名规则统一为 **`sing-box-<version>`**（与 scripts/download-singbox.ts
/// 的部署约定一致），读取侧统一走 `resolve_core_binary`。
///
/// 重要：GUI 与服务端（AuroDaemon）共用同一份数据根目录，因此两边
/// 看到的是同一个 bin/，这是"GUI 写 staging + manifest、服务重启时自更新"
/// 能够成立的前提。
use std::path::{Path, PathBuf};

/// 内核二进制所在目录（各平台统一）
///
/// 与 `get_data_root()` 保持一致：Windows %ProgramData%\Auroweave\bin、
/// macOS ~/Library/Application Support/Auroweave/bin、Linux /var/lib/Auroweave/bin。
pub fn bin_dir() -> PathBuf {
    crate::get_data_root().join("bin")
}

/// 版本号合法性：只允许数字与点，且必须以数字开头
///
/// 该字符串会被拼进文件路径，必须拒绝 `../`、空格、分号等穿越/注入字符。
pub fn is_valid_version(v: &str) -> bool {
    !v.is_empty()
        && v.chars().all(|c| c.is_ascii_digit() || c == '.')
        && v.starts_with(|c: char| c.is_ascii_digit())
}

/// 版本化内核文件名（如 `sing-box-1.14.2` / `sing-box-1.14.2.exe`）
pub fn core_file_name(version: &str) -> String {
    debug_assert!(
        is_valid_version(version),
        "版本号未校验即拼入路径，存在路径穿越风险: {}",
        version
    );
    if cfg!(target_os = "windows") {
        format!("sing-box-{}.exe", version)
    } else {
        format!("sing-box-{}", version)
    }
}


/// 读取侧唯一入口：解析当前应当使用的内核二进制
///
/// 策略：在候选目录中收集所有内核本体，按修改时间取最新。
/// "取最新"而非"精确匹配某个版本"是为了兼容历史遗留的各种命名。
///
/// 候选目录顺序即优先级，命中高优先级目录后不再下探，
/// 保证开发环境 sidecar-bin 的覆盖语义生效：
///   1. 开发环境 sidecar-bin
///   2. 可执行文件同目录（安装后的标准位置）
///   3. 数据根 bin/（服务模式与 macOS 数据目录）
pub fn resolve_core_binary() -> Option<PathBuf> {
    let mut candidate_dirs: Vec<PathBuf> = Vec::new();

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
    #[cfg(target_os = "linux")]
    {
        candidate_dirs.push(PathBuf::from("src-tauri/sidecar-bin/linux-x86_64"));
        candidate_dirs.push(PathBuf::from("src-tauri/sidecar-bin/linux-aarch64"));
        candidate_dirs.push(PathBuf::from("sidecar-bin/linux-x86_64"));
        candidate_dirs.push(PathBuf::from("sidecar-bin/linux-aarch64"));
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidate_dirs.push(exe_dir.to_path_buf());
        }
    }
    candidate_dirs.push(bin_dir());

    let mut latest_path: Option<PathBuf> = None;
    let mut latest_time = std::time::SystemTime::UNIX_EPOCH;

    for dir in candidate_dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name() else { continue };
            if !is_core_file_name(&name.to_string_lossy()) {
                continue;
            }
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(modified) = meta.modified() {
                    if modified > latest_time {
                        latest_time = modified;
                        latest_path = Some(path);
                    }
                }
            }
        }
        if latest_path.is_some() {
            break;
        }
    }

    let path = latest_path?;
    let abs = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&path))
            .unwrap_or(path)
    };
    Some(std::fs::canonicalize(&abs).unwrap_or(abs))
}

/// 写入侧唯一入口：安装内核（版本化命名）并按需回收历史版本
///
/// 顺序很重要：**先写新版本，确认落盘成功后再回收旧的**。
/// 反过来（先删后写）一旦写入失败，用户就彻底没有可回退的内核了。
///
/// 注意：`fs::copy` 以新建文件方式写入，SUID 位必然丢失，
/// 需由调用方按需申请管理员授权恢复（见 SidecarManager::ensure_privileged_binary）。
/// 清理失败（常见于旧文件为 root 属主 + SUID）不视为错误，
/// 仅残留冗余文件，不影响功能。
pub fn install_core_binary(
    src: &Path,
    version: &str,
    keep: Option<&Path>,
) -> std::io::Result<PathBuf> {
    let dir = bin_dir();
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(core_file_name(version));

    std::fs::copy(src, &target)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&target)?.permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(&target, perms);
    }

    if let Some(keep) = keep {
        cleanup_stale_core_binaries(&dir, keep);
    }
    Ok(target)
}

/// 回收 bin/ 中的历史内核本体，保留 `keep` 指定的那个
///
/// 只删内核本体（由 `is_core_file_name` 判定），升级包/配置/日志不受影响。
/// 非递归：子目录不在管辖范围内。
pub fn cleanup_stale_core_binaries(bin_dir: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(bin_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep {
            continue;
        }
        let Some(name) = path.file_name() else { continue };
        if !is_core_file_name(&name.to_string_lossy()) {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(_) => log::info!("[core_paths] 已清理历史内核: {:?}", path),
            Err(e) => log::warn!(
                "[core_paths] 清理历史内核失败（可能需管理员权限，残留不影响功能）: {:?}: {}",
                path,
                e
            ),
        }
    }
}

/// 判断某个文件名是否"是内核本体"（用于识别待回收的历史版本）
///
/// 刻意排除压缩包与占位文件，避免清理时误伤。历史实现写死的
/// `sing-box`（无版本号）也计入——那正是早期升级留下的垃圾。
pub fn is_core_file_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") || lower.ends_with(".zip") || lower.ends_with(".txt") {
        return false;
    }
    if lower == "sing-box" || lower == "sing-box.exe" {
        return true;
    }
    lower.starts_with("sing-box-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_is_versioned() {
        // 统一为版本化命名，与 download-singbox.ts 部署约定一致
        if cfg!(target_os = "windows") {
            assert_eq!(core_file_name("1.14.2"), "sing-box-1.14.2.exe");
        } else {
            assert_eq!(core_file_name("1.14.2"), "sing-box-1.14.2");
        }
    }

    #[test]
    fn core_file_name_detection_covers_legacy_forms() {
        assert!(is_core_file_name("sing-box-1.14.2"));
        assert!(is_core_file_name("sing-box.exe"));
        // 早期实现遗留的无版本号文件——正是要回收的垃圾
        assert!(is_core_file_name("sing-box"));
        // 绝不能误判为内核，否则清理会误伤
        assert!(!is_core_file_name("sing-box-1.14.2-darwin-arm64.tar.gz"));
        assert!(!is_core_file_name("sing-box.zip"));
        assert!(!is_core_file_name("AuroDaemon.exe"));
        assert!(!is_core_file_name("config.json"));
        assert!(!is_core_file_name("README.txt"));
    }

    #[test]
    fn version_validation_rejects_path_traversal() {
        assert!(is_valid_version("1.14.2"));
        // 这些若被放行，会直接拼进文件路径
        assert!(!is_valid_version("1.14.2/../../etc"));
        assert!(!is_valid_version("1.14.2;rm"));
        assert!(!is_valid_version(""));
        assert!(!is_valid_version("v1.14.2"));
        assert!(!is_valid_version(".1"));
    }

    #[test]
    fn bin_dir_ends_with_bin() {
        let dir = bin_dir();
        let joined = dir
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/");
        assert!(joined.ends_with("bin"), "实际: {}", joined);
        assert!(joined.contains("Auroweave"), "实际: {}", joined);
    }

    #[test]
    fn cleanup_keeps_new_version_and_reclaims_legacy() {
        // 真机回归：升级后 bin/ 从 84MB 涨到 160MB，旧内核永不回收
        let dir = std::env::temp_dir().join(format!("auroweave_paths_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in [
            "sing-box-1.14.0",
            "sing-box-1.14.1",
            "sing-box",
            "sing-box-1.14.2-darwin-arm64.tar.gz",
            "config.json",
        ] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        let new_bin = dir.join("sing-box-1.14.2");
        std::fs::write(&new_bin, b"new").unwrap();

        cleanup_stale_core_binaries(&dir, &new_bin);

        assert!(new_bin.exists(), "新内核必须保留");
        assert!(!dir.join("sing-box-1.14.0").exists(), "旧内核应回收");
        assert!(!dir.join("sing-box-1.14.1").exists(), "旧内核应回收");
        assert!(!dir.join("sing-box").exists(), "无版本号残留应回收");
        assert!(dir.join("sing-box-1.14.2-darwin-arm64.tar.gz").exists(), "升级包不应误删");
        assert!(dir.join("config.json").exists(), "配置不应误删");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 与服务端 AuroDaemon 的命名规则必须逐字一致。
    ///
    /// 两个 crate 无法互相依赖（服务端是独立的 Windows-only 二进制），
    /// 因此这条一致性只能靠测试钉住：一旦 GUI 侧改了命名规则而
    /// 服务侧没跟上，GUI 在线升级写入的文件服务侧就认不出来，
    /// 表现为"升级显示成功，服务重启后又回退到旧内核"。
    #[test]
    fn naming_rule_matches_service_side_contract() {
        // AuroDaemon::utils::versioned_core_name / active_core_name 的期望值
        let expected_versioned = if cfg!(target_os = "windows") {
            "sing-box-1.14.2.exe"
        } else {
            "sing-box-1.14.2"
        };
        assert_eq!(core_file_name("1.14.2"), expected_versioned);
        // 稳定入口名（服务侧安装流程使用）
        let expected_stable = if cfg!(target_os = "windows") {
            "sing-box.exe"
        } else {
            "sing-box"
        };
        assert!(is_core_file_name(expected_stable));
    }
}
