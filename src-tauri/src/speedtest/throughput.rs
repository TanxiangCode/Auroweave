/// 单节点吞吐量测速引擎 (HTTP/HTTPS 流式传输分块计时)
/// 作者: TanXiang
use super::ThroughputResult;
use crate::error::AppError;
use reqwest::Proxy;
use std::time::{Duration, Instant};
use tokio::time::timeout;

const DEFAULT_TEST_URL: &str = "https://speed.cloudflare.com/__down?bytes=25000000";

/// 针对单个节点或当前代理，进行限定时长的下载与上传吞吐量测速
pub async fn run_single_throughput_test(
    _node_tag: &str,
    duration_secs: u64,
    mixed_port: u16,
) -> Result<ThroughputResult, AppError> {
    let proxy_url = format!("http://127.0.0.1:{}", mixed_port);
    let proxy = Proxy::all(&proxy_url)
        .map_err(|e| AppError::Network(format!("创建本地代理客户端失败: {}", e)))?;

    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(duration_secs + 5))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    // 1. 下载测速
    let download_bps = measure_download(&client, duration_secs).await.unwrap_or(0);

    // 2. 上传测速 (循环上传分块，计算真实平均吞吐率)
    let upload_bps = measure_upload(&client, duration_secs).await.unwrap_or(0);

    Ok(ThroughputResult {
        download_bps,
        upload_bps,
        tested_at: chrono::Utc::now().timestamp_millis(),
    })
}

async fn measure_download(client: &reqwest::Client, duration_secs: u64) -> Result<u64, AppError> {
    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);

    let res = client
        .get(DEFAULT_TEST_URL)
        .send()
        .await;

    let mut response = match res {
        Ok(r) if r.status().is_success() => r,
        _ => return Ok(0),
    };

    let mut downloaded_bytes: u64 = 0;

    let test_future = async {
        while let Ok(Some(chunk)) = response.chunk().await {
            downloaded_bytes += chunk.len() as u64;
            if start.elapsed() >= duration {
                break;
            }
        }
    };

    let _ = timeout(duration + Duration::from_millis(500), test_future).await;
    let elapsed = start.elapsed().as_secs_f64();

    if elapsed > 0.1 && downloaded_bytes > 0 {
        Ok((downloaded_bytes as f64 / elapsed) as u64)
    } else {
        Ok(0)
    }
}


async fn measure_upload(client: &reqwest::Client, duration_secs: u64) -> Result<u64, AppError> {
    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);
    let mut uploaded_bytes: u64 = 0;

    // 使用 1MB 的块进行循环上传
    let payload = vec![0u8; 1 * 1024 * 1024];

    while start.elapsed() < duration {
        let res = client
            .post("https://speed.cloudflare.com/__up")
            .body(payload.clone())
            .send()
            .await;

        if let Ok(r) = res {
            if r.status().is_success() {
                uploaded_bytes += payload.len() as u64;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();

    if elapsed > 0.1 && uploaded_bytes > 0 {
        Ok((uploaded_bytes as f64 / elapsed) as u64)
    } else {
        Ok(0)
    }
}
