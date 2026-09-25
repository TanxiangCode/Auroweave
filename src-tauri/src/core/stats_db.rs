/// 流量历史统计与应用追踪的本地数据库管理
/// 作者: TanXiang
use rusqlite::{Connection, Result};
use std::collections::HashMap;
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
    conn.pragma_update(None, "synchronous", "NORMAL")?;    // 流量每小时聚合表
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

    // 节点解锁检测历史表（服务状态随时间留痕，重启不丢；
    // services 为 JSON：{"gemini":"yes","claude":"no"}）
    conn.execute(
        "CREATE TABLE IF NOT EXISTS unlock_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            node_tag TEXT NOT NULL,
            services TEXT NOT NULL,
            egress_ip TEXT,
            country_code TEXT,
            hosting INTEGER, /* null=未查询 */
            proxy_flag INTEGER,
            isp TEXT,
            tested_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_unlock_node_time ON unlock_history(node_tag, tested_at DESC)",
        [],
    )?;

    // 历史保留期裁剪（plan-O O-3）：unlock/speedtest 两表只增不删会无限膨胀，
    // 且趋势查询本就 LIMIT 20——90 天外数据无消费者。开库一次，幂等（<10ms）
    if let Err(e) = prune_history(conn) {
        log::warn!("[stats_db] 历史保留期裁剪失败（不影响使用，下次开库重试）: {}", e);
    }

    Ok(())
}

/// 历史保留期（天）：解锁状态与测速记录的可用价值窗口
const HISTORY_RETENTION_DAYS: i64 = 90;

/// 删除 tested_at 早于 90 天的历史行（unlock_history + speedtest_history）
fn prune_history(conn: &Connection) -> Result<()> {
    let cutoff = chrono::Utc::now().timestamp_millis() - HISTORY_RETENTION_DAYS * 24 * 3600 * 1000;
    conn.execute(
        "DELETE FROM unlock_history WHERE tested_at < ?1",
        rusqlite::params![cutoff],
    )?;
    conn.execute(
        "DELETE FROM speedtest_history WHERE tested_at < ?1",
        rusqlite::params![cutoff],
    )?;
    Ok(())
}

/// 按节点 tag 批量删除历史记录（订阅删除时联动清理其节点数据，
/// 防止孤儿记录——tag 永远查不到消费者）
pub fn delete_history_by_tags(tags: &[String]) {
    if tags.is_empty() {
        return;
    }
    if let Err(e) = with_conn(|conn| {
        conn.execute_batch("BEGIN;")?;
        {
            let mut stmt = conn.prepare("DELETE FROM unlock_history WHERE node_tag = ?1")?;
            for tag in tags {
                stmt.execute(rusqlite::params![tag])?;
            }
        }
        {
            let mut stmt = conn.prepare("DELETE FROM speedtest_history WHERE node_tag = ?1")?;
            for tag in tags {
                stmt.execute(rusqlite::params![tag])?;
            }
        }
        conn.execute_batch("COMMIT;")?;
        Ok(())
    }) {
        log::warn!("[stats_db] 订阅删除联动清理历史失败（孤儿数据容忍，不影响功能）: {}", e);
    }
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
             DELETE FROM unlock_history;
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

/// 从已打开连接查询每个节点的最近吞吐结果（供真实 DB 与内存单测复用）。
fn query_latest_speedtest_per_node(
    conn: &Connection,
) -> Result<HashMap<String, SpeedtestRecord>> {
    let mut stmt = conn.prepare(
        "SELECT node_tag, download_bps, upload_bps, delay_ms, tested_at
         FROM speedtest_history
         WHERE delay_ms IS NULL
         ORDER BY tested_at ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(SpeedtestRecord {
            node_tag: row.get(0)?,
            download_bps: row.get::<_, i64>(1)? as u64,
            upload_bps: row.get::<_, i64>(2)? as u64,
            delay_ms: row.get::<_, Option<i64>>(3)?.map(|d| d as u64),
            tested_at: row.get(4)?,
        })
    })?;

    let mut latest = HashMap::new();
    for row in rows {
        let record = row?;
        latest.insert(record.node_tag.clone(), record);
    }
    Ok(latest)
}

/// 每个节点取最近一次吞吐测速结果（应用启动时回填 store 用）。
/// 仅选择 delay_ms IS NULL 的真实吞吐记录；按时间升序覆盖，后写入的即最新结果。
pub fn get_latest_speedtest_per_node() -> Result<HashMap<String, SpeedtestRecord>> {
    with_conn(query_latest_speedtest_per_node)
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

// ==================== 解锁检测历史 ====================

/// 解锁检测历史记录（持久化后的查询视图，结构与 UnlockCheckResult 对齐）
#[derive(Debug, serde::Serialize)]
pub struct UnlockRecord {
    pub node_tag: String,
    /// 服务状态 JSON：{"gemini":"yes","claude":"no","chatgpt":"risky"}
    pub services: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub egress_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_flag: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isp: Option<String>,
    pub tested_at: i64,
}

/// 写入一条解锁检测记录（失败仅记日志）
pub fn add_unlock_record(rec: &crate::core::unlock_check::UnlockCheckResult) {
    if let Err(e) = with_conn(|conn| {
        conn.execute(
            "INSERT INTO unlock_history
                (node_tag, services, egress_ip, country_code, hosting, proxy_flag, isp, tested_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                rec.node_tag,
                serde_json::to_string(&rec.services).unwrap_or_else(|_| "{}".to_string()),
                rec.egress_ip,
                rec.country_code,
                rec.hosting.map(|b| b as i64),
                rec.proxy_flag.map(|b| b as i64),
                rec.isp,
                rec.tested_at,
            ],
        )?;
        Ok(())
    }) {
        log::warn!("[stats_db] 写入解锁检测历史失败: {}", e);
    }
}

/// 批量写入解锁检测记录（单事务；批量检测收尾/攒批场景使用）
pub fn add_unlock_records_batch(records: Vec<crate::core::unlock_check::UnlockCheckResult>) {
    if records.is_empty() {
        return;
    }
    if let Err(e) = with_conn(|conn| {
        conn.execute_batch("BEGIN;")?;
        {
            let mut stmt = conn.prepare(
                "INSERT INTO unlock_history
                    (node_tag, services, egress_ip, country_code, hosting, proxy_flag, isp, tested_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for rec in &records {
                stmt.execute(rusqlite::params![
                    rec.node_tag,
                    serde_json::to_string(&rec.services).unwrap_or_else(|_| "{}".to_string()),
                    rec.egress_ip,
                    rec.country_code,
                    rec.hosting.map(|b| b as i64),
                    rec.proxy_flag.map(|b| b as i64),
                    rec.isp,
                    rec.tested_at,
                ])?;
            }
        }
        conn.execute_batch("COMMIT;")?;
        Ok(())
    }) {
        log::warn!("[stats_db] 批量写入解锁检测历史失败: {}", e);
    }
}

/// 每个节点取最近一次解锁检测结果（应用启动时回填 store 用；
/// 全表只扫每个 tag 的最新行，数百节点量级一次完成）
pub fn get_latest_unlock_per_node() -> Result<Vec<UnlockRecord>> {
    with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT u.node_tag, u.services, u.egress_ip, u.country_code, u.hosting, u.proxy_flag, u.isp, u.tested_at
             FROM unlock_history u
             INNER JOIN (
                 SELECT node_tag, MAX(tested_at) AS max_ts
                 FROM unlock_history
                 GROUP BY node_tag
             ) latest ON u.node_tag = latest.node_tag AND u.tested_at = latest.max_ts",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(UnlockRecord {
                node_tag: row.get(0)?,
                services: serde_json::from_str(&row.get::<_, String>(1)?)
                    .unwrap_or(serde_json::Value::Null),
                egress_ip: row.get(2)?,
                country_code: row.get(3)?,
                hosting: row.get::<_, Option<i64>>(4)?.map(|b| b != 0),
                proxy_flag: row.get::<_, Option<i64>>(5)?.map(|b| b != 0),
                isp: row.get(6)?,
                tested_at: row.get(7)?,
            })
        })?;
        rows.collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// in-memory 连接跑完整 init_schema（不触真实 DB_CONN），验证裁剪与联动语义
    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn insert_unlock(conn: &Connection, tag: &str, tested_at: i64) {
        conn.execute(
            "INSERT INTO unlock_history (node_tag, services, tested_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![tag, "{}", tested_at],
        )
        .unwrap();
    }

    fn insert_speedtest(conn: &Connection, tag: &str, tested_at: i64) {
        conn.execute(
            "INSERT INTO speedtest_history (node_tag, download_bps, upload_bps, delay_ms, tested_at) VALUES (?1, 0, 0, NULL, ?2)",
            rusqlite::params![tag, tested_at],
        )
        .unwrap();
    }

    fn count_rows(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn prune_removes_rows_older_than_90_days() {
        let conn = test_conn();
        let now = chrono::Utc::now().timestamp_millis();
        let stale = now - 91 * 24 * 3600 * 1000;
        let fresh = now - 89 * 24 * 3600 * 1000;
        insert_unlock(&conn, "旧节点", stale);
        insert_unlock(&conn, "新节点", fresh);
        insert_speedtest(&conn, "旧节点", stale);
        insert_speedtest(&conn, "新节点", fresh);

        prune_history(&conn).unwrap();

        assert_eq!(count_rows(&conn, "unlock_history"), 1);
        assert_eq!(count_rows(&conn, "speedtest_history"), 1);
        // 剩余的是新行
        let tag: String = conn
            .query_row("SELECT node_tag FROM unlock_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tag, "新节点");
    }

    #[test]
    fn latest_speedtest_per_node_keeps_newest_throughput_record() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO speedtest_history
                (node_tag, download_bps, upload_bps, delay_ms, tested_at)
             VALUES ('a', 100, 10, NULL, 100)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO speedtest_history
                (node_tag, download_bps, upload_bps, delay_ms, tested_at)
             VALUES ('a', 900, 90, NULL, 300)",
            [],
        )
        .unwrap();
        // 纯延迟记录不得污染吞吐结果视图。
        conn.execute(
            "INSERT INTO speedtest_history
                (node_tag, download_bps, upload_bps, delay_ms, tested_at)
             VALUES ('a', 0, 0, 25, 500)",
            [],
        )
        .unwrap();

        let latest = query_latest_speedtest_per_node(&conn).unwrap();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest["a"].download_bps, 900);
        assert_eq!(latest["a"].upload_bps, 90);
        assert_eq!(latest["a"].tested_at, 300);
    }

    #[test]
    fn delete_by_tags_removes_only_matching() {
        let conn = test_conn();
        let now = chrono::Utc::now().timestamp_millis();
        for tag in ["a", "b", "c"] {
            insert_unlock(&conn, tag, now);
            insert_speedtest(&conn, tag, now);
        }
        // with_conn 走全局 DB_CONN；此处直接内联等价逻辑验证 SQL 语义
        let tags = vec!["a".to_string(), "c".to_string()];
        conn.execute_batch("BEGIN;").unwrap();
        {
            let mut stmt = conn.prepare("DELETE FROM unlock_history WHERE node_tag = ?1").unwrap();
            for tag in &tags {
                stmt.execute(rusqlite::params![tag]).unwrap();
            }
        }
        conn.execute_batch("COMMIT;").unwrap();

        assert_eq!(count_rows(&conn, "unlock_history"), 1);
        let tag: String = conn
            .query_row("SELECT node_tag FROM unlock_history", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tag, "b");
    }
}
