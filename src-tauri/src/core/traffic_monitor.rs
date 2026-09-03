/// 流量与应用程序级数据采集监控任务
/// 作者: TanXiang
use std::collections::HashMap;
use tauri::AppHandle;
use tokio::time::{sleep, Duration};
use crate::commands::settings::settings_get_internal;
use crate::core::stats_db::{add_traffic_delta, add_app_traffic_delta};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionMetadata {
    process_path: Option<String>,
    process: Option<String>,
}

#[derive(serde::Deserialize)]
struct Connection {
    id: String,
    metadata: ConnectionMetadata,
    upload: u64,
    download: u64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionsResponse {
    upload_total: u64,
    download_total: u64,
    connections: Vec<Connection>,
}

pub fn start_monitor(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_total_up = 0u64;
        let mut last_total_down = 0u64;

        // 记录每个连接上次看到的流量 (id -> (up, down))
        let mut conn_history: HashMap<String, (u64, u64)> = HashMap::new();
        // 追踪开关上一轮状态：重新开启后的首个周期只记录基线，不计算增量，
        // 避免把连接自建立以来的历史流量重复计入当前小时（双计数）
        let mut tracking_was_enabled = false;
        // 无超时的客户端在内核 TCP 建立但不再响应时会永久挂起整个监控循环；
        // 必须携带 Authorization Bearer 头（内核启用了随机 secret，裸请求恒 401，
        // 会导致应用级流量统计整体失效）
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                if let Ok(v) = reqwest::header::HeaderValue::from_str(&format!(
                    "Bearer {}",
                    crate::core::clash_api::get_clash_api_secret()
                )) {
                    h.insert(reqwest::header::AUTHORIZATION, v);
                }
                h
            })
            .build()
            .unwrap_or_default();

        loop {
            let settings = settings_get_internal(&app_handle);
            let port = settings.clash_api_port;
            let tracking_enabled = settings.enable_app_traffic_tracking;
            let first_cycle_after_reenable = tracking_enabled && !tracking_was_enabled;
            tracking_was_enabled = tracking_enabled;

            // 当前所在的小时的时间戳 (整点)
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let current_hour = now - (now % 3600);

            let url = format!("http://127.0.0.1:{}/connections", port);

            match client.get(&url).send().await {
                Ok(resp) => {
                    if let Ok(data) = resp.json::<ConnectionsResponse>().await {
                        // 1. 处理总流量
                        // 由于内核重启会导致 Total 清零，所以如果现在的 Total 比上次小，说明重启过
                        let delta_up = if data.upload_total >= last_total_up { data.upload_total - last_total_up } else { data.upload_total };
                        let delta_down = if data.download_total >= last_total_down { data.download_total - last_total_down } else { data.download_total };

                        if delta_up > 0 || delta_down > 0 {
                            add_traffic_delta(current_hour, delta_down, delta_up);
                        }

                        last_total_up = data.upload_total;
                        last_total_down = data.download_total;

                        // 2. 处理各应用的流量 (只在开启追踪时记录)
                        if tracking_enabled {
                            let mut current_ids = std::collections::HashSet::new();
                            let mut app_deltas: HashMap<String, (u64, u64)> = HashMap::new();

                            for conn in data.connections {
                                current_ids.insert(conn.id.clone());

                                let process_name = conn.metadata.process_path
                                    .or(conn.metadata.process)
                                    .unwrap_or_else(|| "Unknown".to_string());

                                // 提取进程名，如 C:\Program Files\Chrome\chrome.exe -> chrome.exe
                                let process_name = std::path::Path::new(&process_name)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(&process_name)
                                    .to_string();

                                // 重开追踪后的首个周期：仅记录基线，不计增量（防止双计数）
                                if first_cycle_after_reenable {
                                    conn_history.insert(conn.id, (conn.upload, conn.download));
                                    continue;
                                }

                                let (last_up, last_down) = conn_history.get(&conn.id).unwrap_or(&(0, 0));

                                let d_up = if conn.upload >= *last_up { conn.upload - last_up } else { conn.upload };
                                let d_down = if conn.download >= *last_down { conn.download - last_down } else { conn.download };

                                if d_up > 0 || d_down > 0 {
                                    let entry = app_deltas.entry(process_name).or_insert((0, 0));
                                    entry.0 += d_up;
                                    entry.1 += d_down;
                                }

                                conn_history.insert(conn.id, (conn.upload, conn.download));
                            }

                            // 写入数据库
                            for (proc, (up, down)) in app_deltas {
                                add_app_traffic_delta(current_hour, &proc, down, up);
                            }

                            // 清理已经关闭的连接
                            conn_history.retain(|id, _| current_ids.contains(id));
                        } else {
                            // 如果关闭追踪，清空历史状态避免下次开启时产生巨大的突变
                            conn_history.clear();
                        }
                    }
                }
                Err(_) => {
                    // 无法连接内核（可能是未启动），将最后记录的值清零，以防下次内核启动时产生负数或巨大的跳变
                    last_total_up = 0;
                    last_total_down = 0;
                    conn_history.clear();
                }
            }

            // 每 10 秒采集一次
            sleep(Duration::from_secs(10)).await;
        }
    });
}
