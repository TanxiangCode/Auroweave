/// IPC 命令 — 智能测速
/// 作者: TanXiang
use crate::core::clash_api::ClashApiClient;
use crate::error::{ApiResponse, AppError};
use crate::speedtest::scheduler::SpeedTestScheduler;
use crate::speedtest::throughput::run_single_throughput_test;
use crate::speedtest::ThroughputResult;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tracing::info;

/// 触发延迟测速（调用 ClashAPI /proxies/{tag}/delay 触发测试，使用配置的 Semaphore 动态并发）
///
/// 返回 HashMap<String, u16>：
/// - delay > 0：测速成功，值为延迟毫秒数
/// - delay = 0：测速失败（超时或不可达），前端可区分「已测试但失败」与「未测试」
#[tauri::command]
pub async fn speedtest_run_latency(
    app_handle: AppHandle,
    group_tag: String,
    node_tags: Vec<String>,
) -> Result<ApiResponse<HashMap<String, u16>>, AppError> {
    let settings = crate::commands::settings::settings_get_internal(&app_handle);
    let concurrency = (settings.latency_test_concurrency as usize).clamp(1, 100);

    let timeout_ms = settings.latency_test_timeout_ms.clamp(500, 30000);
    let test_url = if settings.latency_test_url.trim().is_empty() {
        "http://www.gstatic.com/generate_204".to_string()
    } else {
        settings.latency_test_url.clone()
    };

    info!(
        "触发 [{}] 分组共 {} 个节点的受控并发延迟测试 (并发上限: {}, 超时: {}ms, URL: {})",
        group_tag,
        node_tags.len(),
        concurrency,
        timeout_ms,
        test_url
    );

    // 空节点列表直接报错，避免启动"0 个节点"的空批次让前端误以为测试完成
    if node_tags.is_empty() {
        return Ok(ApiResponse::err(
            AppError::Validation("节点列表为空，无需执行延迟测试".to_string()),
            400,
        ));
    }

    let clash_client = Arc::new(ClashApiClient::default());
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut join_set = tokio::task::JoinSet::new();

    for (idx, tag) in node_tags.into_iter().enumerate() {
        let client = clash_client.clone();
        let sem = semaphore.clone();
        let url = test_url.clone();
        join_set.spawn(async move {
            if crate::core::parser::is_announcement_or_fake_node(&tag, None, None) {
                return (tag, 0u16);
            }
            let _permit = sem.acquire().await.ok();
            if idx > 0 && idx % 10 == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(15)).await;
            }
            match client.get_node_delay(&tag, &url, timeout_ms).await {
                Ok(delay) => (tag, delay),
                Err(e) => {
                    log::debug!("[speedtest] 节点 [{}] 延迟测试失败: {}", tag, e);
                    (tag, 0u16)
                }
            }
        });
    }


    let mut results = HashMap::new();
    while let Some(res) = join_set.join_next().await {
        if let Ok((tag, delay)) = res {
            results.insert(tag, delay);
        }
    }

    // 若当前为 auto / urltest 策略组，显式触发 sing-box 内核进行策略组级优选刷新
    if !group_tag.is_empty() && (group_tag == "auto" || group_tag == "balance" || group_tag.ends_with("-auto")) {
        let client = clash_client.clone();
        let gt = group_tag.clone();
        let url = test_url.clone();
        tokio::spawn(async move {
            let _ = client.trigger_urltest_group_delay(&gt, &url, timeout_ms).await;
            log::info!("[speedtest] 已触发 URLTest 策略组 [{}] 内部最优节点重选", gt);
        });
    }

    info!("延迟测试完成: 成功 {} / 失败 {} / 总计 {}",
        results.values().filter(|&&d| d > 0).count(),
        results.values().filter(|&&d| d == 0).count(),
        results.len()
    );


    Ok(ApiResponse::ok(results))
}

/// 单节点吞吐量测速
#[tauri::command]
pub async fn speedtest_run_single(
    app_handle: tauri::AppHandle,
    node_tag: String,
) -> Result<ApiResponse<ThroughputResult>, AppError> {
    let node_tag = node_tag.trim().to_string();
    if node_tag.is_empty() {
        return Ok(ApiResponse::err(
            AppError::Validation("节点名称不能为空".to_string()),
            400,
        ));
    }
    info!("开始对节点 [{}] 运行单体吞吐量测速...", node_tag);
    let port = crate::speedtest::get_mixed_port(&app_handle);

    // 吞吐测速经本地 mixed 端口发起，流量走 selector 当前选中节点。
    // 单节点测速必须先把所在 selector 组切换到目标节点，否则测的是
    // 用户当前选中的其他节点（历史缺陷：测速结果张冠李戴）。
    // 通过 /proxies 找到包含该节点的 selector 组并切换，测速后还原原选中节点。
    let client = ClashApiClient::default();
    let mut restored: Option<(String, String)> = None; // (group_tag, original_now)
    if let Ok(proxies) = client.get_proxies().await {
        if let Some(proxies) = proxies.get("proxies").and_then(|p| p.as_object()) {
            let group = proxies.iter().find(|(_, v)| {
                v.get("type").and_then(|t| t.as_str()) == Some("Selector")
                    && v.get("all").and_then(|a| a.as_array())
                        .map(|a| a.iter().any(|t| t.as_str() == Some(node_tag.as_str())))
                        .unwrap_or(false)
            });
            if let Some((g_tag, g_val)) = group {
                let original_now = g_val.get("now").and_then(|n| n.as_str()).unwrap_or("").to_string();
                if original_now != node_tag {
                    if let Err(e) = client.select_node(g_tag, &node_tag).await {
                        return Ok(ApiResponse::err(format!("无法切换到目标节点进行测速: {}", e), 500));
                    }
                    restored = Some((g_tag.clone(), original_now));
                    info!("单节点测速: [{}] 临时切换 selector [{}]（结束后还原）", node_tag, g_tag);
                }
            } else {
                log::warn!("[speedtest] 节点 [{}] 不在任何 Selector 组中，按当前出站测速", node_tag);
            }
        }
    }

    let result = run_single_throughput_test(&node_tag, 5, port).await;

    // 还原用户原选中节点
    if let Some((g_tag, original_now)) = restored {
        if !original_now.is_empty() {
            if let Err(e) = client.select_node(&g_tag, &original_now).await {
                log::warn!("[speedtest] 还原节点选择失败: {}", e);
            }
        }
    }

    match result {
        Ok(res) => Ok(ApiResponse::ok(res)),
        Err(e) => Ok(ApiResponse::err(format!("单节点测速失败: {}", e), 500)),
    }
}

/// 批量测速（串行调度）
#[tauri::command]
pub async fn speedtest_run_batch(
    app_handle: AppHandle,
    scheduler: State<'_, Arc<SpeedTestScheduler>>,
    group_tag: String,
    node_tags: Vec<String>,
) -> Result<ApiResponse<()>, AppError> {
    // 空节点列表直接报错，避免启动空批次（调度器会立刻"完成"让前端误判成功）
    if node_tags.is_empty() {
        return Ok(ApiResponse::err(
            AppError::Validation("节点列表为空，无法启动批量测速".to_string()),
            400,
        ));
    }
    info!("启动批量测速队列，分组: {}, 节点数量: {}", group_tag, node_tags.len());
    scheduler.run_batch(app_handle, group_tag, node_tags).await;
    Ok(ApiResponse::ok(()))
}

/// 取消批量测速
#[tauri::command]
pub async fn speedtest_cancel_batch(
    scheduler: State<'_, Arc<SpeedTestScheduler>>,
) -> Result<ApiResponse<()>, AppError> {
    info!("取消正在运行的批量测速任务");
    scheduler.cancel();
    Ok(ApiResponse::ok(()))
}

/// 获取已缓存的测速结果
#[tauri::command]
pub async fn speedtest_get_results(
    scheduler: State<'_, Arc<SpeedTestScheduler>>,
) -> Result<ApiResponse<HashMap<String, ThroughputResult>>, AppError> {
    Ok(ApiResponse::ok(scheduler.get_results()))
}
