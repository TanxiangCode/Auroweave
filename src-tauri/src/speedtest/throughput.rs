/// 单节点吞吐量测速引擎 (HTTP/HTTPS 流式传输分块计时)
/// 作者: TanXiang
use super::ThroughputResult;
use crate::error::AppError;
use reqwest::Proxy;
use std::time::{Duration, Instant};
use tokio::time::timeout;

const DEFAULT_TEST_URL: &str = "https://speed.cloudflare.com/__down?bytes=25000000";

/// 针对单个节点或当前代理，进行限定时长的下载与上传吞吐量测速
///
/// `test_url` 为用户设置的下载数据源（空串回退 Cloudflare 默认）；
/// 上传统一走 Cloudflare /__up（生态内无通用上传端点，不暴露为设置）。
pub async fn run_single_throughput_test_with_url(
    _node_tag: &str,
    duration_secs: u64,
    mixed_port: u16,
    test_url: &str,
) -> Result<ThroughputResult, AppError> {
    let test_url = if test_url.trim().is_empty() {
        DEFAULT_TEST_URL
    } else {
        test_url.trim()
    };

    let proxy_url = format!("http://127.0.0.1:{}", mixed_port);
    let proxy = Proxy::all(&proxy_url)
        .map_err(|e| AppError::Network(format!("创建本地代理客户端失败: {}", e)))?;

    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(duration_secs + 5))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    // 1. 下载测速（失败记日志，与真实 0 带宽区分）
    let download_bps = match measure_download(&client, duration_secs, test_url).await {
        Ok(b) => b,
        Err(e) => {
            log::warn!("[throughput] 下载测速失败: {}", e);
            0
        }
    };

    // 2. 上传测速 (循环上传分块，计算真实平均吞吐率)
    let upload_bps = match measure_upload(&client, duration_secs).await {
        Ok(b) => b,
        Err(e) => {
            log::warn!("[throughput] 上传测速失败: {}", e);
            0
        }
    };

    Ok(ThroughputResult {
        download_bps,
        upload_bps,
        tested_at: chrono::Utc::now().timestamp_millis(),
    })
}

async fn measure_download(client: &reqwest::Client, duration_secs: u64, test_url: &str) -> Result<u64, AppError> {
    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);

    let res = client
        .get(test_url)
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

    // 使用 1MB 的块进行循环上传。
    // reqwest::Body 会夺取所有权，因此用全局静态切片构造，避免每轮 clone 分配 1MB，
    // 也避免历史实现的 Box::leak 在批量测速（每节点一次调用）中累积泄漏。
    static PAYLOAD: std::sync::OnceLock<&'static [u8]> = std::sync::OnceLock::new();
    let payload: &'static [u8] = *PAYLOAD.get_or_init(|| {
        Box::leak(vec![0u8; 1024 * 1024].into_boxed_slice())
    });

    while start.elapsed() < duration {
        let res = client
            .post("https://speed.cloudflare.com/__up")
            .body(reqwest::Body::from(payload))
            .send()
            .await;

        if let Ok(r) = res {
            if r.status().is_success() {
                uploaded_bytes += payload.len() as u64;
            } else {
                log::warn!("[throughput] 上传测速收到非成功状态: {}", r.status());
                break;
            }
        } else {
            log::warn!("[throughput] 上传测速请求失败: 网络错误（区别于真实 0 带宽）");
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
