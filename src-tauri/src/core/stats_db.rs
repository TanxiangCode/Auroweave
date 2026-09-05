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
    // WAL 模式：写不阻塞读（流量监控写拍与前端统计查询并发不再串行竞争），
    // synchronous=NORMAL 在 WAL 下安全且大幅减少 fsync（默认 FULL 每条 autocommit 一次）
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
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

    // 节点测速历史表（单测结果按节点+时间留痕，重启不丢）
    conn.execute(
        "CREATE TABLE IF NOT EXISTS speedtest_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_tag TEXT NOT NULL,
            download_bps INTEGER NOT NULL DEFAULT 0,
            upload_bps INTEGER NOT NULL DEFAULT 0,
            delay_ms INTEGER, /* null=未测延迟 */
            tested_at INTEGER NOT NULL /* 毫秒时间戳 */
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_speedtest_node_time ON speedtest_history(node_tag, tested_at DESC)",
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
             DELETE FROM speedtest_history;
             COMMIT;",
        )?;
        Ok(())
    })
}

/// 测速历史记录（持久化后的查询视图）
#[derive(Debug, serde::Serialize)]
pub struct SpeedtestRecord {
    pub node_tag: String,
    pub download_bps: u64,
    pub upload_bps: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u64>,
    pub tested_at: i64,
}

/// 写入一条测速记录（吞吐或延迟任一即可；失败仅记日志）
pub fn add_speedtest_record(node_tag: &str, download_bps: u64, upload_bps: u64, delay_ms: Option<u64>) {
    if let Err(e) = with_conn(|conn| {
        conn.execute(
            "INSERT INTO speedtest_history (node_tag, download_bps, upload_bps, delay_ms, tested_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                node_tag,
                download_bps as i64,
                upload_bps as i64,
                delay_ms.map(|d| d as i64),
                chrono::Utc::now().timestamp_millis()
            ],
        )?;
        Ok(())
    }) {
        log::warn!("[stats_db] 写入测速历史失败: {}", e);
    }
}

/// 批量写入测速记录（单事务 + 一次 fsync；延迟测速攒批等场景使用）
///
/// 逐条调用 add_speedtest_record 时每条为独立 autocommit（WAL 下每次 fsync），
/// 300 节点批量场景合并为单事务提交。
pub fn add_speedtest_records_batch(records: Vec<(String, u64, u64, Option<u64>)>) {
    if records.is_empty() {
        return;
    }
    if let Err(e) = with_conn(|conn| {
        // 事务由 with_conn 外层连接保证原子性；失败整体回滚（攒批场景可整批重试）
        conn.execute_batch("BEGIN;")?;
        {
            let mut stmt = conn.prepare(
                "INSERT INTO speedtest_history (node_tag, download_bps, upload_bps, delay_ms, tested_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (tag, download_bps, upload_bps, delay_ms) in &records {
                stmt.execute(rusqlite::params![
                    tag,
                    *download_bps as i64,
                    *upload_bps as i64,
                    delay_ms.map(|d| d as i64),
                    chrono::Utc::now().timestamp_millis()
                ])?;
            }
        }
        conn.execute_batch("COMMIT;")?;
        Ok(())
    }) {
        log::warn!("[stats_db] 批量写入测速历史失败: {}", e);
    }
}

/// 查询指定节点最近 limit 条测速历史（时间倒序）
///
/// 排除纯延迟测试写入的零吞吐记录（download_bps=0 AND upload_bps=0 AND delay_ms
/// 非空）——它们用于节点存活趋势，混入会把吞吐趋势图画成 0 速度拐点。
/// 真实吞吐测速失败时 dl/ul 同为 0 但 delay_ms 为 NULL，予以保留。
pub fn get_speedtest_history(node_tag: &str, limit: u32) -> Result<Vec<SpeedtestRecord>> {
    with_conn(|conn| {
        let limit = limit.clamp(1, 100) as i32;
        let mut stmt = conn.prepare(
            "SELECT node_tag, download_bps, upload_bps, delay_ms, tested_at
             FROM speedtest_history
             WHERE node_tag = ?1
               AND NOT (download_bps = 0 AND upload_bps = 0 AND delay_ms IS NOT NULL)
             ORDER BY tested_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![node_tag, limit], |row| {
            Ok(SpeedtestRecord {
                node_tag: row.get(0)?,
                download_bps: row.get::<_, i64>(1)? as u64,
                upload_bps: row.get::<_, i64>(2)? as u64,
                delay_ms: row.get::<_, Option<i64>>(3)?.map(|d| d as u64),
                tested_at: row.get(4)?,
            })
        })?;
        rows.collect()
    })
}
