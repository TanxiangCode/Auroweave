/// 共享工具函数模块
/// 提供跨模块复用的辅助函数，避免代码重复
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
        let hash = hasher.finalize();
        return hash.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    }
    String::new()
}


