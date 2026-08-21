/// IPC 命令 — 流量统计数据获取
/// 作者: TanXiang
use crate::error::ApiResponse;
use crate::core::stats_db::DB_CONN;
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

#[tauri::command]
pub async fn get_traffic_history(dimension: String) -> ApiResponse<Vec<TrafficSeriesPoint>> {
    let conn = match DB_CONN.lock() {
        Ok(guard) => guard,
        Err(_) => return ApiResponse::err("无法连接到本地数据库".to_string(), 500),
    };

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let mut results = Vec::new();

    // 考虑到 SQLite 的跨平台简便性，这里在 Rust 端生成需要的时间区间，然后通过 SQL 查询求和。
    // 为了简单起见，如果是在内存不足或极简情况，我们生成时间槽并执行查询。

    match dimension.as_str() {
        "day" => {
            // 近 24 小时
            for i in (0..24).rev() {
                let start_time = now - (now % 3600) - (i * 3600);
                let end_time = start_time + 3600;
                
                let mut stmt = conn.prepare("SELECT SUM(download_bytes), SUM(upload_bytes) FROM traffic_hourly WHERE timestamp_hour >= ? AND timestamp_hour < ?").unwrap();
                let mut rows = stmt.query(rusqlite::params![start_time, end_time]).unwrap();
                
                let (mut down, mut up) = (0, 0);
                if let Some(row) = rows.next().unwrap() {
                    down = row.get::<_, i64>(0).unwrap_or(0) as u64;
                    up = row.get::<_, i64>(1).unwrap_or(0) as u64;
                }

                let dt = chrono::DateTime::from_timestamp(start_time, 0).unwrap();
                // 格式化输出为 HH:00，例如 08:00
                let label = format!("{:02}:00", dt.format("%H"));
                results.push(TrafficSeriesPoint { label, download_bytes: down, upload_bytes: up });
            }
        }
        "month" => {
            // 近 30 天
            let current_day_start = now - (now % 86400); // UTC based day start roughly
            for i in (0..30).rev() {
                let start_time = current_day_start - (i * 86400);
                let end_time = start_time + 86400;
                
                let mut stmt = conn.prepare("SELECT SUM(download_bytes), SUM(upload_bytes) FROM traffic_hourly WHERE timestamp_hour >= ? AND timestamp_hour < ?").unwrap();
                let mut rows = stmt.query(rusqlite::params![start_time, end_time]).unwrap();
                
                let (mut down, mut up) = (0, 0);
                if let Some(row) = rows.next().unwrap() {
                    down = row.get::<_, i64>(0).unwrap_or(0) as u64;
                    up = row.get::<_, i64>(1).unwrap_or(0) as u64;
                }

                // 简单的号数，由于本地时区差异，这里用 chrono 转换为 Local
                let dt = chrono::DateTime::from_timestamp(start_time, 0).unwrap().with_timezone(&chrono::Local);
                let label = format!("{}号", dt.format("%d"));
                results.push(TrafficSeriesPoint { label, download_bytes: down, upload_bytes: up });
            }
        }
        "year" => {
            // 近 12 个月 (近似，按每 30 天计算或者直接在应用层计算)
            // 真实日历月比较复杂，简化为按过去 365 天切分为 12 份，或者直接在 SQL 中根据 strftime 聚合
            let current_day_start = now - (now % 86400); 
            for i in (0..12).rev() {
                // 每份 30 天
                let start_time = current_day_start - (i * 30 * 86400) - (30 * 86400);
                let end_time = start_time + (30 * 86400);
                
                let mut stmt = conn.prepare("SELECT SUM(download_bytes), SUM(upload_bytes) FROM traffic_hourly WHERE timestamp_hour >= ? AND timestamp_hour < ?").unwrap();
                let mut rows = stmt.query(rusqlite::params![start_time, end_time]).unwrap();
                
                let (mut down, mut up) = (0, 0);
                if let Some(row) = rows.next().unwrap() {
                    down = row.get::<_, i64>(0).unwrap_or(0) as u64;
                    up = row.get::<_, i64>(1).unwrap_or(0) as u64;
                }

                let dt = chrono::DateTime::from_timestamp(start_time + 15 * 86400, 0).unwrap().with_timezone(&chrono::Local);
                let label = format!("{}月", dt.format("%m"));
                results.push(TrafficSeriesPoint { label, download_bytes: down, upload_bytes: up });
            }
        }
        _ => return ApiResponse::err("不支持的查询维度".to_string(), 400),
    }

    ApiResponse::ok(results)
}

#[tauri::command]
pub async fn get_app_traffic_stats() -> ApiResponse<Vec<AppTrafficPoint>> {
    let conn = match DB_CONN.lock() {
        Ok(guard) => guard,
        Err(_) => return ApiResponse::err("无法连接到本地数据库".to_string(), 500),
    };

    // 近 24 小时
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let start_time = now - 86400;

    let mut stmt = conn.prepare(
        "SELECT process_name, SUM(download_bytes) as down, SUM(upload_bytes) as up 
         FROM app_traffic_hourly 
         WHERE timestamp_hour >= ? 
         GROUP BY process_name 
         ORDER BY (down + up) DESC LIMIT 10"
    ).unwrap();

    let rows = stmt.query_map(rusqlite::params![start_time], |row| {
        Ok(AppTrafficPoint {
            process_name: row.get(0)?,
            download_bytes: row.get::<_, i64>(1).unwrap_or(0) as u64,
            upload_bytes: row.get::<_, i64>(2).unwrap_or(0) as u64,
        })
    }).unwrap();

    let mut results = Vec::new();
    for row in rows {
        if let Ok(r) = row {
            results.push(r);
        }
    }

    ApiResponse::ok(results)
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

