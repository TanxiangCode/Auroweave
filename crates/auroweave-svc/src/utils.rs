/// 共享工具函数模块
/// 提供跨模块复用的辅助函数，避免代码重复
use std::path::PathBuf;
use sha2::{Sha256, Digest};

/// 计算文件的 SHA-256 哈希值（十六进制小写）
///
/// 使用 64KB 缓冲区读取文件，平衡内存占用和 I/O 效率。
/// 文件无法打开或读取失败时返回空字符串。
pub fn compute_sha256(path: &std::path::Path) -> String {
    if let Ok(mut file) = std::fs::File::open(path) {
        let mut hasher = Sha256::new();
        // 64KB 缓冲区，显著提升大文件的磁盘 I/O 读取速度
        let mut buffer = [0u8; 65536];
        loop {
            match std::io::Read::read(&mut file, &mut buffer) {
                Ok(0) => break,
                Ok(n) => hasher.update(&buffer[..n]),
                Err(_) => return String::new(),
            }
        }
        return format!("{:x}", hasher.finalize());
    }
    String::new()
}

/// 多路径候选自动匹配 sing-box 二进制路径
///
/// 搜索顺序：
/// 1. 当前可执行文件同目录下的 `sing-box.exe`
/// 2. ProgramData/Auroweave/sing-box.exe（服务安装时复制的位置）
/// 3. --singbox 参数指定的路径（由调用方处理）
///
/// 返回第一个找到的路径，未找到时返回错误。
pub fn resolve_binary_path() -> Result<PathBuf, String> {
    let mut candidate_dirs = Vec::new();

    // 1. 当前可执行文件同目录
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidate_dirs.push(exe_dir.to_path_buf());
        }
    }

    // 2. ProgramData/Auroweave（服务安装时复制的位置）
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    candidate_dirs.push(PathBuf::from(program_data).join("Auroweave"));

    // 按优先级搜索 sing-box.exe
    for dir in &candidate_dirs {
        let singbox_path = dir.join("sing-box.exe");
        if singbox_path.exists() {
            return Ok(singbox_path);
        }
    }

    Err("未能在候选目录中找到 sing-box.exe".to_string())
}
