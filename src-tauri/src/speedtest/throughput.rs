/// 单节点吞吐量测速引擎 (HTTP/HTTPS 流式传输分块计时)
/// 作者: TanXiang
use super::ThroughputResult;
use crate::error::AppError;
use reqwest::Proxy;
use std::time::{Duration, Instant};
use tokio::time::timeout;

const DEFAULT_TEST_URL: &str = "https://speed.cloudflare.com/__down?bytes=25000000";
const LOCAL_PROXY_URL: &str = "http://127.0.0.1:7890";

/// 针对单个节点或当前代理，进行限定时长的下载与上传吞吐量测速
pub async fn run_single_throughput_test(
    _node_tag: &str,
    duration_secs: u64,
) -> Result<ThroughputResult, AppError> {
    let proxy = Proxy::all(LOCAL_PROXY_URL)
        .map_err(|e| AppError::Network(format!("创建本地代理客户端失败: {}", e)))?;

    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(duration_secs + 5))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    // 1. 下载测速
    let download_bps = measure_download(&client, duration_secs).await.unwrap_or(0);

    // 2. 上传测速 (使用 5MB 随机数据 payload)
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
        Ok(r) => r,
        Err(_) => return Ok(0),
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

    let _ = timeout(duration, test_future).await;
    let elapsed = start.elapsed().as_secs_f64();

    if elapsed > 0.1 {
        Ok((downloaded_bytes as f64 / elapsed) as u64)
    } else {
        Ok(0)
    }
}

async fn measure_upload(client: &reqwest::Client, duration_secs: u64) -> Result<u64, AppError> {
    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);

    // 生成 2MB 随机测试数据
    let payload = vec![0u8; 2 * 1024 * 1024];

    let res = client
        .post("https://speed.cloudflare.com/__up")
        .body(payload.clone())
        .send();

    let mut uploaded_bytes: u64 = 0;

    let test_future = async {
        if let Ok(r) = res.await {
            if r.status().is_success() {
                uploaded_bytes += payload.len() as u64;
            }
        }
    };

    let _ = timeout(duration, test_future).await;
    let elapsed = start.elapsed().as_secs_f64();

    if elapsed > 0.1 && uploaded_bytes > 0 {
        Ok((uploaded_bytes as f64 / elapsed) as u64)
    } else {
        Ok(0)
    }
}
