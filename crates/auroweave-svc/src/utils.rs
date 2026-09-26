/// 共享工具函数模块
/// 提供跨模块复用的辅助函数，避免代码重复
use sha2::{Sha256, Digest};

// ============================================================================
// 内核二进制路径规则（与 GUI 侧 `auroweave_lib::core_paths` 保持一致）
// ============================================================================
//
// AuroDaemon 是独立的 crate（不能依赖 Tauri lib），但它与 GUI 必须对
// "内核叫什么名字、放在哪"达成完全一致，否则会出现：
//   - GUI 在线升级写入 `sing-box-1.14.3.exe`，本模块却只读/只写
//     `sing-box.exe`，于是 GUI 的升级结果被自更新流程无视（或反过来
//     被 manifest 覆盖回旧版），两边互相打架；
//   - 两个模块各自维护一份 `.../Auroweave/bin` 拼接逻辑，改一处漏一处。
//
// 命名规则统一为 `sing-box-<version>`，与 scripts/download-singbox.ts
// 及 GUI 侧 core_paths 一致。`active_core_name()` 仍返回无版本号的
// `sing-box.exe` 作为"稳定入口"名，供 manifest 比对与安装流程使用——
// 这样既保留服务侧"覆盖同一个文件"的安全模型，又让读取侧
// （resolve_core_binary，按 mtime 取最新）能认出 GUI 写入的版本化文件。

/// 数据根目录 `%ProgramData%\Auroweave`
pub fn data_root() -> std::path::PathBuf {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    std::path::PathBuf::from(program_data).join("Auroweave")
}

/// 内核二进制目录 `%ProgramData%\Auroweave\bin`
pub fn bin_dir() -> std::path::PathBuf {
    data_root().join("bin")
}

/// 服务侧使用的稳定入口文件名（manifest 哈希比对 / 覆盖更新的目标）
pub fn active_core_name() -> String {
    "sing-box.exe".to_string()
}

/// 版本化内核文件名（与 GUI 侧 core_file_name 同规则）
///
/// 目前仅由测试引用：它记录的是"两侧必须一致"的命名契约本身。
/// 服务实际运行时只用稳定入口名 `sing-box.exe`。
#[allow(dead_code)]
pub fn versioned_core_name(version: &str) -> String {
    format!("sing-box-{}.exe", version)
}

/// 判断文件名是否为"内核本体"（用于识别待回收的历史版本）
pub fn is_core_file_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".zip") || lower.ends_with(".tar.gz") || lower.ends_with(".txt") {
        return false;
    }
    lower == "sing-box" || lower == "sing-box.exe" || lower.starts_with("sing-box-")
}

/// 读取侧唯一入口：在候选目录中按 mtime 取最新的内核二进制
///
/// 候选目录：可执行文件同目录（服务安装位置）优先，其次数据根 bin/。
/// "取最新"而非精确匹配版本，是为了同时兼容稳定入口名与 GUI 写入的版本化文件名。
pub fn resolve_core_binary() -> Option<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
        }
    }
    dirs.push(bin_dir());

    let mut latest: Option<std::path::PathBuf> = None;
    let mut latest_time = std::time::SystemTime::UNIX_EPOCH;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let Some(name) = path.file_name() else { continue };
            if !is_core_file_name(&name.to_string_lossy()) { continue; }
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(modified) = meta.modified() {
                    if modified > latest_time {
                        latest_time = modified;
                        latest = Some(path);
                    }
                }
            }
        }
        if latest.is_some() { break; }
    }
    let p = latest?;
    Some(std::fs::canonicalize(&p).unwrap_or(p))
}

/// 供 installer/updater 复用的"在目录中找最新内核"实现
///
/// installer 原本自己写了一份 read_dir + 前缀匹配 + mtime 比较，
/// 与 core_manager 的查找逻辑重复且易漂移。这里收敛为唯一实现。
pub fn newest_core_in_dir(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut latest: Option<std::path::PathBuf> = None;
    let mut latest_time = std::time::SystemTime::UNIX_EPOCH;
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
                    latest = Some(path);
                }
            }
        }
    }
    latest
}

/// 回收 bin/ 中的历史内核本体，保留 keep 指定的那个
///
/// GUI 在线升级写的是 `sing-box-<version>.exe`；服务安装流程写的是
/// 无版本号的稳定入口名。两者共存会让 bin/ 随每次升级单调膨胀，
/// 这里在安装/更新成功后把其余内核本体回收掉。
pub fn cleanup_stale_core_binaries(bin_dir: &std::path::Path, keep: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(bin_dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep {
            continue;
        }
        let Some(name) = path.file_name() else { continue };
        if !is_core_file_name(&name.to_string_lossy()) {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            tracing::info!("已清理历史内核: {:?}", path);
        }
    }
}

/// 计算文件的 SHA-256 哈希值（十六进制小写）
///
/// 使用 64KB 缓冲区读取文件，平衡内存占用和 I/O 效率。
/// 文件无法打开或读取失败时返回 `None`，调用方必须显式处理失败
/// （例如 updater 对 `None` 直接跳过更新），而不是把空字符串当作合法哈希参与比对。
pub fn compute_sha256(path: &std::path::Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    // 64KB 缓冲区，显著提升大文件的磁盘 I/O 读取速度
    let mut buffer = [0u8; 65536];
    loop {
        match std::io::Read::read(&mut file, &mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(_) => return None,
        }
    }
    let hash = hasher.finalize();
    Some(hash.iter().map(|b| format!("{:02x}", b)).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 内核命名规则必须与 GUI 侧 `auroweave_lib::core_paths::core_file_name`
    /// 逐字一致。两边一旦漂移，GUI 在线升级写入的文件服务侧就认不出来，
    /// 表现为"升级成功但服务重启后又回到旧内核"。
    #[test]
    fn versioned_name_matches_gui_rule() {
        assert_eq!(versioned_core_name("1.14.2"), "sing-box-1.14.2.exe");
        assert_eq!(active_core_name(), "sing-box.exe");
    }

    #[test]
    fn core_file_detection_excludes_archives() {
        assert!(is_core_file_name("sing-box-1.14.2.exe"));
        assert!(is_core_file_name("sing-box.exe"));
        assert!(!is_core_file_name("sing-box.zip"));
        assert!(!is_core_file_name("sing-box.tar.gz"));
        assert!(!is_core_file_name("AuroDaemon.exe"));
        assert!(!is_core_file_name("config.json"));
    }

    #[test]
    fn cleanup_reclaims_versioned_leftovers() {
        // GUI 升级写 sing-box-1.14.2.exe，服务写 sing-box.exe，
        // 共存会导致 bin/ 膨胀且服务按 mtime 选错内核
        let dir = std::env::temp_dir().join(format!("auroweave_svc_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for n in ["sing-box-1.14.1.exe", "sing-box-1.14.2.exe", "config.json"] {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        let keep = dir.join("sing-box.exe");
        std::fs::write(&keep, b"stable").unwrap();

        cleanup_stale_core_binaries(&dir, &keep);

        assert!(keep.exists(), "稳定入口必须保留");
        assert!(!dir.join("sing-box-1.14.2.exe").exists(), "版本化残留应回收");
        assert!(!dir.join("sing-box-1.14.1.exe").exists());
        assert!(dir.join("config.json").exists(), "配置不应误删");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
