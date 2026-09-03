/// 文件系统工具：原子写入与持久化串行化
/// 作者: TanXiang
///
/// 所有 JSON 持久化（settings.json / subscriptions.json / config.json）
/// 必须通过本模块写入，避免"截断重写"中途崩溃导致文件损坏。
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

/// 全局持久化锁：串行化 settings / subscriptions 的读-改-写事务，
/// 防止托盘菜单、前端并发保存、自动更新调度器互相覆盖。
static PERSIST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn persist_lock() -> &'static Mutex<()> {
    PERSIST_LOCK.get_or_init(|| Mutex::new(()))
}

/// 获取持久化事务锁的守卫。锁中毒时强取（前持锁者已 panic，数据未损坏则继续）。
pub fn acquire_persist_lock() -> MutexGuard<'static, ()> {
    match persist_lock().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// 原子写文件：先写入同目录临时文件，再 rename 原子替换。
/// rename 在同一文件系统内是原子操作；写入中途崩溃只会留下 .tmp 残留，不会损坏原文件。
pub fn atomic_write(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "路径缺少父目录"))?;
    fs::create_dir_all(parent)?;
    let tmp_path = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(contents)?;
        f.sync_all()?;
    }
    fs::rename(&tmp_path, path)?;
    Ok(())
}

/// 便捷方法：原子写 JSON
pub fn atomic_write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|e| format!("序列化失败: {}", e))?;
    atomic_write(path, content.as_bytes()).map_err(|e| format!("写入 {:?} 失败: {}", path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_creates_and_replaces() {
        let dir = std::env::temp_dir().join("auroweave_fs_utils_test");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        atomic_write(&path, b"{\"a\":1}").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"a\":1}");
        atomic_write(&path, b"{\"a\":2}").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"a\":2}");
        let _ = fs::remove_dir_all(&dir);
    }
}
