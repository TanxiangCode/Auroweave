/// 流量历史统计与应用追踪的本地数据库管理
/// 作者: TanXiang
use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::get_data_root;

lazy_static::lazy_static! {
    pub static ref DB_CONN: Arc<Mutex<Option<Connection>>> = {
        let db_path = get_db_path();
        // 确保目录存在
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // 打开失败（磁盘满/权限/文件损坏）时降级为 None：统计功能静默禁用，不再 panic 拖垮后端
        let conn = match Connection::open(&db_path).and_then(|c| { init_schema(&c)?; Ok(c) }) {
            Ok(c) => Some(c),
            Err(e) => {
                log::error!("[stats_db] 初始化流量数据库失败，统计功能已禁用: {}", e);
                None
            }
        };
        Arc::new(Mutex::new(conn))
    };
}

fn get_db_path() -> PathBuf {
    get_data_root().join("data").join("stats.dat")
}

fn init_schema(conn: &Connection) -> Result<()> {
    // 流量每小时聚合表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS traffic_hourly (
            timestamp_hour INTEGER PRIMARY KEY, /* Unix timestamp for the start of the hour */
            download_bytes INTEGER NOT NULL DEFAULT 0,
            upload_bytes INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;

    // 应用程序流量每小时聚合表
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_traffic_hourly (
            timestamp_hour INTEGER,
            process_name TEXT,
            download_bytes INTEGER NOT NULL DEFAULT 0,
            upload_bytes INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (timestamp_hour, process_name)
        )",
        [],
    )?;

    Ok(())
}

/// 获取数据库连接。数据库不可用时返回 Err（调用方决定记录日志或忽略）。
pub fn with_conn<T>(f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let guard = match DB_CONN.lock() {
        Ok(g) => g,
        // 锁中毒：前持锁者已 panic，强取继续（连接本身未损坏）
        Err(poisoned) => poisoned.into_inner(),
    };
    let conn = guard.as_ref().ok_or_else(|| rusqlite::Error::ToSqlConversionFailure(
        Box::new(std::io::Error::new(std::io::ErrorKind::NotConnected, "流量数据库不可用"))
    ))?;
    f(conn)
}

/// 写入流量增量（失败仅记录日志，不打断流量监控循环）
pub fn add_traffic_delta(timestamp_hour: i64, download_delta: u64, upload_delta: u64) {
    if let Err(e) = with_conn(|conn| {
        conn.execute(
            "INSERT INTO traffic_hourly (timestamp_hour, download_bytes, upload_bytes)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(timestamp_hour) DO UPDATE SET
             download_bytes = download_bytes + excluded.download_bytes,
             upload_bytes = upload_bytes + excluded.upload_bytes",
            rusqlite::params![timestamp_hour, download_delta as i64, upload_delta as i64],
        )?;
        Ok(())
    }) {
        log::warn!("[stats_db] 写入流量增量失败: {}", e);
    }
}

/// 写入应用程序流量增量（失败仅记录日志）
pub fn add_app_traffic_delta(timestamp_hour: i64, process_name: &str, download_delta: u64, upload_delta: u64) {
    if let Err(e) = with_conn(|conn| {
        conn.execute(
            "INSERT INTO app_traffic_hourly (timestamp_hour, process_name, download_bytes, upload_bytes)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(timestamp_hour, process_name) DO UPDATE SET
             download_bytes = download_bytes + excluded.download_bytes,
             upload_bytes = upload_bytes + excluded.upload_bytes",
            rusqlite::params![timestamp_hour, process_name, download_delta as i64, upload_delta as i64],
        )?;
        Ok(())
    }) {
        log::warn!("[stats_db] 写入应用流量增量失败: {}", e);
    }
}

/// 清空所有历史流量统计记录（事务包裹，避免部分清空）
pub fn clear_all_stats() -> Result<()> {
    with_conn(|conn| {
        conn.execute_batch(
            "BEGIN;
             DELETE FROM traffic_hourly;
             DELETE FROM app_traffic_hourly;
             COMMIT;",
        )?;
        Ok(())
    })
}
