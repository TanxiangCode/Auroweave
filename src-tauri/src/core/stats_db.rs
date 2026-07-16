/// 流量历史统计与应用追踪的本地数据库管理
/// 作者: TanXiang
use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::get_data_root;

lazy_static::lazy_static! {
    pub static ref DB_CONN: Arc<Mutex<Connection>> = {
        let db_path = get_db_path();
        // 确保目录存在
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&db_path).expect("无法打开或创建流量数据库");
        init_schema(&conn).expect("初始化数据库表结构失败");
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

/// 写入流量增量
pub fn add_traffic_delta(timestamp_hour: i64, download_delta: u64, upload_delta: u64) -> Result<()> {
    let conn = DB_CONN.lock().unwrap();
    conn.execute(
        "INSERT INTO traffic_hourly (timestamp_hour, download_bytes, upload_bytes)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(timestamp_hour) DO UPDATE SET
         download_bytes = download_bytes + excluded.download_bytes,
         upload_bytes = upload_bytes + excluded.upload_bytes",
        rusqlite::params![timestamp_hour, download_delta as i64, upload_delta as i64],
    )?;
    Ok(())
}

/// 写入应用程序流量增量
pub fn add_app_traffic_delta(timestamp_hour: i64, process_name: &str, download_delta: u64, upload_delta: u64) -> Result<()> {
    let conn = DB_CONN.lock().unwrap();
    conn.execute(
        "INSERT INTO app_traffic_hourly (timestamp_hour, process_name, download_bytes, upload_bytes)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(timestamp_hour, process_name) DO UPDATE SET
         download_bytes = download_bytes + excluded.download_bytes,
         upload_bytes = upload_bytes + excluded.upload_bytes",
        rusqlite::params![timestamp_hour, process_name, download_delta as i64, upload_delta as i64],
    )?;
    Ok(())
}
