/// IPC 命令 — 流量统计数据获取
/// 作者: TanXiang
use crate::error::ApiResponse;
use crate::core::stats_db::with_conn;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct TrafficSeriesPoint {
    pub label: String, // 时间标签, 例如 "12:00", "5号", "10月"
    pub download_bytes: u64,
    pub upload_bytes: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct AppTrafficPoint {
    pub process_name: String,
    pub download_bytes: u64,
    pub upload_bytes: u64,
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn get_traffic_history(dimension: String) -> ApiResponse<Vec<TrafficSeriesPoint>> {
    let query_result = with_conn(|conn| {
        let now = now_unix();
        let mut results = Vec::new();

        match dimension.as_str() {
            "day" => {
                // 近 24 小时
                let mut stmt = conn.prepare(
                    "SELECT SUM(download_bytes), SUM(upload_bytes) FROM traffic_hourly WHERE timestamp_hour >= ? AND timestamp_hour < ?"
                )?;
                for i in (0..24).rev() {
                    let start_time = now - (now % 3600) - (i * 3600);
                    let end_time = start_time + 3600;

                    let (down, up) = {
                        let mut rows = stmt.query(rusqlite::params![start_time, end_time])?;
                        if let Some(row) = rows.next()? {
                            (
                                row.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as u64,
                                row.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as u64,
                            )
                        } else {
                            (0, 0)
                        }
                    };

                    if let Some(dt) = chrono::DateTime::from_timestamp(start_time, 0) {
                        let label = format!("{:02}:00", dt.format("%H"));
                        results.push(TrafficSeriesPoint { label, download_bytes: down, upload_bytes: up });
                    }
                }
            }
            "month" => {
                // 近 30 天
                let current_day_start = now - (now % 86400);
                let mut stmt = conn.prepare(
                    "SELECT SUM(download_bytes), SUM(upload_bytes) FROM traffic_hourly WHERE timestamp_hour >= ? AND timestamp_hour < ?"
                )?;
                for i in (0..30).rev() {
                    let start_time = current_day_start - (i * 86400);
                    let end_time = start_time + 86400;

                    let (down, up) = {
                        let mut rows = stmt.query(rusqlite::params![start_time, end_time])?;
                        if let Some(row) = rows.next()? {
                            (
                                row.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as u64,
                                row.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as u64,
                            )
                        } else {
                            (0, 0)
                        }
                    };

                    if let Some(dt) = chrono::DateTime::from_timestamp(start_time, 0) {
                        let dt = dt.with_timezone(&chrono::Local);
                        let label = format!("{}号", dt.format("%d"));
                        results.push(TrafficSeriesPoint { label, download_bytes: down, upload_bytes: up });
                    }
                }
            }
            "year" => {
                // 近 12 个月（每份按 30 天近似）
                let current_day_start = now - (now % 86400);
                let mut stmt = conn.prepare(
                    "SELECT SUM(download_bytes), SUM(upload_bytes) FROM traffic_hourly WHERE timestamp_hour >= ? AND timestamp_hour < ?"
                )?;
                for i in (0..12).rev() {
                    let start_time = current_day_start - (i * 30 * 86400) - (30 * 86400);
                    let end_time = start_time + (30 * 86400);

                    let (down, up) = {
                        let mut rows = stmt.query(rusqlite::params![start_time, end_time])?;
                        if let Some(row) = rows.next()? {
                            (
                                row.get::<_, Option<i64>>(0)?.unwrap_or(0).max(0) as u64,
                                row.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as u64,
                            )
                        } else {
                            (0, 0)
                        }
                    };

                    if let Some(dt) = chrono::DateTime::from_timestamp(start_time + 15 * 86400, 0) {
                        let dt = dt.with_timezone(&chrono::Local);
                        let label = format!("{}月", dt.format("%m"));
                        results.push(TrafficSeriesPoint { label, download_bytes: down, upload_bytes: up });
                    }
                }
            }
            _ => return Err(rusqlite::Error::InvalidParameterName("不支持的查询维度".to_string())),
        }

        Ok(results)
    });

    match query_result {
        Ok(results) => ApiResponse::ok(results),
        Err(e) => ApiResponse::err(format!("查询流量历史失败: {}", e), 500),
    }
}

#[tauri::command]
pub async fn get_app_traffic_stats() -> ApiResponse<Vec<AppTrafficPoint>> {
    let query_result = with_conn(|conn| {
        // 近 24 小时
        let now = now_unix();
        let start_time = now - 86400;

        let mut stmt = conn.prepare(
            "SELECT process_name, SUM(download_bytes) as down, SUM(upload_bytes) as up
             FROM app_traffic_hourly
             WHERE timestamp_hour >= ?
             GROUP BY process_name
             ORDER BY (down + up) DESC LIMIT 10"
        )?;

        let rows = stmt.query_map(rusqlite::params![start_time], |row| {
            Ok(AppTrafficPoint {
                process_name: row.get(0)?,
                download_bytes: row.get::<_, Option<i64>>(1)?.unwrap_or(0).max(0) as u64,
                upload_bytes: row.get::<_, Option<i64>>(2)?.unwrap_or(0).max(0) as u64,
            })
        })?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    });

    match query_result {
        Ok(results) => ApiResponse::ok(results),
        Err(e) => ApiResponse::err(format!("查询应用流量统计失败: {}", e), 500),
    }
}

/// 清空历史流量统计与应用流量数据表
#[tauri::command]
pub async fn stats_clear_all() -> ApiResponse<()> {
    match crate::core::stats_db::clear_all_stats() {
        Ok(_) => {
            log::info!("[stats] 历史流量统计数据已全部清空");
            ApiResponse::ok(())
        }
        Err(e) => ApiResponse::err(format!("清空历史流量数据失败: {}", e), 500),
    }
}
