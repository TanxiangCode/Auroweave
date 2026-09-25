/// 单节点吞吐量测速引擎 (HTTP/HTTPS 流式传输分块计时)
/// 作者: TanXiang
use super::ThroughputResult;
use crate::error::AppError;
use reqwest::Proxy;
use std::time::{Duration, Instant};
use tokio::time::timeout;

const LEGACY_DEFAULT_SPEED_URL: &str =
    "https://speed.cloudflare.com/__down?bytes=25000000";
const DEFAULT_SPEED_URL: &str = "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-aarch64-apple-darwin.tar.gz";
const DEFAULT_SPEED_URLS: &[&str] = &[
    DEFAULT_SPEED_URL,
    "https://ash-speed.hetzner.com/100MB.bin",
    "https://proof.ovh.net/files/10Mb.dat",
    LEGACY_DEFAULT_SPEED_URL,
];

fn download_candidates(test_url: &str) -> Vec<String> {
    let primary = if test_url.trim().is_empty() {
        DEFAULT_SPEED_URL
    } else {
        test_url.trim()
    };
    let mut urls = vec![primary.to_string()];
    if DEFAULT_SPEED_URLS.contains(&primary) {
        for url in DEFAULT_SPEED_URLS {
            if *url != primary
                && !urls
                    .iter()
                    .any(|item: &String| item.as_str() == *url)
            {
                urls.push((*url).to_string());
            }
        }
    }
    urls
}

fn endpoint_label(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "custom".to_string())
}

/// 针对单个节点或当前代理，进行限定时长的下载与上传吞吐量测速
///
/// `egress_port`：流量出口的本地 mixed 端口——主实例（selector 已被调用方
/// 切到目标节点）或 test-core 专属端口（inbound 规则已钉死到目标节点）。
/// `test_url` 为用户设置的下载数据源（空串回退 GitHub 默认）；内置预设按不同
/// 服务商自动回退，自定义地址只使用用户填写值。上传仍沿用 Cloudflare /__up。
/// `parallel_updown`：上下行并行（O-6，settings.speedtest_parallel_updown；
/// test-core 专属端口下互不干扰；主 mixed 端口下会互相挤占带宽）。
pub async fn run_single_throughput_test_with_url(
    _node_tag: &str,
    duration_secs: u64,
    egress_port: u16,
    test_url: &str,
    parallel_updown: bool,
) -> Result<ThroughputResult, AppError> {
    let test_urls = download_candidates(test_url);

    let proxy_url = format!("http://127.0.0.1:{}", egress_port);
    let proxy = Proxy::all(&proxy_url)
        .map_err(|e| AppError::Network(format!("创建本地代理客户端失败: {}", e)))?;

    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(duration_secs + 5))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    if parallel_updown {
        // 上下行并行：互不等待，总耗时 ≈ max(下行, 上行)（test-core 专属端口
        // 下两路独立节点出站；主 mixed 端口下共享出口会互相挤占）
        let (dl, ul) = tokio::join!(
            async {
                measure_download_with_fallback(&client, duration_secs, &test_urls).await
            },
            async {
                measure_upload(&client, duration_secs)
                    .await
                    .unwrap_or_else(|e| {
                        log::warn!("[throughput] 上传测速失败: {}", e);
                        0
                    })
            }
        );
        return Ok(ThroughputResult {
            download_bps: dl,
            upload_bps: ul,
            tested_at: chrono::Utc::now().timestamp_millis(),
        });
    }

    // 1. 下载测速（失败记日志，与真实 0 带宽区分）
    let download_bps = measure_download_with_fallback(&client, duration_secs, &test_urls).await;

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

async fn measure_download_with_fallback(
    client: &reqwest::Client,
    duration_secs: u64,
    test_urls: &[String],
) -> u64 {
    for (index, url) in test_urls.iter().enumerate() {
        match measure_download(client, duration_secs, url).await {
            Ok(bps) if bps > 0 => {
                if index > 0 {
                    log::info!(
                        "[throughput] 主测速源不可用，已切换备用源: {}",
                        endpoint_label(url)
                    );
                }
                return bps;
            }
            Ok(_) => log::warn!(
                "[throughput] 下载测速源未返回数据: {}",
                endpoint_label(url)
            ),
            Err(e) => log::warn!(
                "[throughput] 下载测速源不可用 [{}]: {}",
                endpoint_label(url),
                e
            ),
        }
    }
    log::warn!("[throughput] 所有下载测速源均不可用");
    0
}

async fn measure_download(
    client: &reqwest::Client,
    duration_secs: u64,
    test_url: &str,
) -> Result<u64, AppError> {
    let start = Instant::now();
    let duration = Duration::from_secs(duration_secs);
    let mut response = client.get(test_url).send().await.map_err(|e| {
        AppError::Network(format!("下载测速请求失败: {}", e))
    })?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "下载测速返回非成功状态: {}",
            status
        )));
    }

    let mut downloaded_bytes: u64 = 0;
    let transfer_result = timeout(duration + Duration::from_millis(500), async {
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    downloaded_bytes += chunk.len() as u64;
                    if start.elapsed() >= duration {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }
        Ok::<(), reqwest::Error>(())
    })
    .await;

    if let Err(e) = transfer_result {
        if downloaded_bytes == 0 {
            return Err(AppError::Network(format!("下载测速传输失败: {}", e)));
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    if elapsed > 0.1 && downloaded_bytes > 0 {
        Ok((downloaded_bytes as f64 / elapsed) as u64)
    } else {
        Err(AppError::Network("下载测速未收到有效数据".to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_download_adds_cross_provider_fallbacks() {
        let urls = download_candidates(DEFAULT_SPEED_URL);
        assert_eq!(urls[0], DEFAULT_SPEED_URL);
        assert_eq!(urls.len(), DEFAULT_SPEED_URLS.len());
        assert!(urls.iter().any(|url| url == "https://ash-speed.hetzner.com/100MB.bin"));
    }

    #[test]
    fn custom_download_source_is_respected_without_overriding() {
        let urls = download_candidates("https://example.com/speed.bin");
        assert_eq!(urls, vec!["https://example.com/speed.bin"]);
    }

    #[tokio::test]
    #[ignore = "requires Sparkle proxy on 127.0.0.1:7890 and network access"]
    async fn live_default_download_source_returns_data() {
        let result = run_single_throughput_test_with_url(
            "live-smoke",
            2,
            7890,
            DEFAULT_SPEED_URL,
            false,
        )
        .await
        .unwrap();
        println!(
            "live download: {:.2} MiB/s",
            result.download_bps as f64 / 1024.0 / 1024.0
        );
        assert!(result.download_bps > 0);
    }
}
