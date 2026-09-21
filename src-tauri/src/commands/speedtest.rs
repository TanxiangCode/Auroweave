/// IPC 命令 — 智能测速
/// 作者: TanXiang
use crate::core::clash_api::ClashApiClient;
use crate::error::{ApiResponse, AppError};
use crate::speedtest::scheduler::SpeedTestScheduler;
use crate::speedtest::throughput::run_single_throughput_test_with_url;
use crate::speedtest::ThroughputResult;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tracing::info;

/// 批量延迟测试取消标志
static LATENCY_CANCELLED: AtomicBool = AtomicBool::new(false);

/// 批量延迟测试实时进度事件负载
#[derive(Clone, serde::Serialize)]
pub struct LatencyProgressPayload {
    pub current_index: usize,
    pub total: usize,
    pub current_node: String,
    pub delay: u16,
}

/// 取消当前正在进行的批量延迟测速
#[tauri::command]
pub async fn speedtest_cancel_latency() -> Result<ApiResponse<()>, AppError> {
    LATENCY_CANCELLED.store(true, Ordering::Relaxed);
    info!("[speedtest] 收到取消批量延迟测试指令");
    Ok(ApiResponse::ok(()))
}

/// 触发延迟测速（调用 ClashAPI /proxies/{tag}/delay 触发测试，使用配置的 Semaphore 动态并发，削峰平滑调度）
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
    // 削峰限制：测延迟默认并发收敛在 16，上限 32，防止大批量随机子域并发轰炸触发公共 DNS 限流
    let concurrency = (settings.latency_test_concurrency as usize).clamp(1, 32);

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

    // 重置取消标志
    LATENCY_CANCELLED.store(false, Ordering::Relaxed);

    // 测速前轻量刷新本地系统 DNS 缓存（环境自洁，杜绝旧客户端 Fake-IP 残留导致 15 秒假死）
    crate::system::sysproxy::flush_system_dns_cache();

    let total = node_tags.len();
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

            // 平滑启动调度：每个节点按索引间隔 25ms 平滑推入（上限 2000ms），削平瞬时流量毛刺
            let stagger_ms = (idx as u64 * 25).min(2000);
            if stagger_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(stagger_ms)).await;
            }

            // 检查是否已收到取消指令
            if LATENCY_CANCELLED.load(Ordering::Relaxed) {
                return (tag, 0u16);
            }

            let _permit = sem.acquire().await.ok();
            if LATENCY_CANCELLED.load(Ordering::Relaxed) {
                return (tag, 0u16);
            }

            let mut delay = match client.get_node_delay(&tag, &url, timeout_ms).await {
                Ok(d) => d,
                Err(e) => {
                    log::debug!("[speedtest] 节点 [{}] 延迟测试初次失败: {}", tag, e);
                    0u16
                }
            };

            // 偶发抖动退避重试（若超时且未取消，等待 400ms 退避重试 1 次，过滤网络瞬态丢包）
            if delay == 0 && !LATENCY_CANCELLED.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                if !LATENCY_CANCELLED.load(Ordering::Relaxed) {
                    if let Ok(retry_d) = client.get_node_delay(&tag, &url, timeout_ms).await {
                        delay = retry_d;
                    }
                }
            }

            (tag, delay)
        });
    }

    let mut results = HashMap::new();
    let mut pending_records: Vec<(String, u16)> = Vec::new();
    let mut current_index = 0;

    while let Some(res) = join_set.join_next().await {
        if let Ok((tag, delay)) = res {
            current_index += 1;

            // 实时向前端广播批量测延迟进度
            let _ = app_handle.emit(
                "latency-test-progress",
                LatencyProgressPayload {
                    current_index: current_index.min(total),
                    total,
                    current_node: tag.clone(),
                    delay,
                },
            );

            // 持久化延迟历史（0=失败也留痕，可看节点存活趋势）——先攒批
            pending_records.push((tag.clone(), delay));
            results.insert(tag, delay);
        }
    }

    // 攒批写库：300 节点原逐条 fsync，合并为单事务（spawn_blocking 不阻塞 async worker）
    if !pending_records.is_empty() {
        tokio::task::spawn_blocking(move || {
            crate::core::stats_db::add_speedtest_records_batch(
                pending_records
                    .into_iter()
                    .map(|(tag, delay)| (tag, 0, 0, Some(delay as u64)))
                    .collect(),
            );
        });
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

/// 单节点吞吐量测速（test-core 零打扰路径；订阅池未命中降级 selector 切换）
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
    let mixed_port = crate::speedtest::get_mixed_port(&app_handle);
    // 用户设置的测速数据源（设置页 speed_test_url；空串回退内置 Cloudflare）
    let settings = crate::commands::settings::settings_get_internal(&app_handle);
    let test_url = settings.speed_test_url.clone();
    let parallel_updown = settings.speedtest_parallel_updown;
    let port_base = if settings.test_core_port_base > 0 {
        settings.test_core_port_base
    } else {
        crate::core::test_core::DEFAULT_PORT_BASE
    };

    // ---- 主路径：订阅池命中 → test-core 专属端口（selector/用户流量零打扰） ----
    if let Some(node) = crate::commands::subscription::collect_active_outbounds()
        .unwrap_or_default()
        .into_iter()
        .find(|n| n.tag == node_tag && crate::core::parser::is_valid_proxy_node(n))
    {
        let core = crate::core::test_core::TestCoreManager::new();
        // 单节点错开 +500 端口段：与并发批量批次互不干扰（不持全局锁）
        match core
            .spawn(std::slice::from_ref(&node), crate::core::test_core::single_node_port_base(port_base))
            .await
        {
            Ok(base) => {
                let result = run_single_throughput_test_with_url(&node_tag, 5, base, &test_url, parallel_updown).await;
                core.stop().await; // 测速完立即销毁
                match result {
                    Ok(res) => {
                        crate::core::stats_db::add_speedtest_record(
                            &node_tag,
                            res.download_bps,
                            res.upload_bps,
                            None,
                        );
                        info!("[speedtest] 单节点测速完成（test-core）: [{}]", node_tag);
                        return Ok(ApiResponse::ok(res));
                    }
                    Err(e) => return Ok(ApiResponse::err(format!("单节点测速失败: {}", e), 500)),
                }
            }
            Err(e) => {
                // 拉起失败降级 selector 路径（下方继续）
                log::warn!("[speedtest] test-core 拉起失败，单节点测速降级 selector 路径: {}", e);
            }
        }
    }

    // ---- 降级路径：selector 临时切换（v1 语义） ----
    // 吞吐测速经本地 mixed 端口发起，流量走 selector 当前选中节点。
    // 单节点测速必须先把所在 selector 组切换到目标节点，否则测的是
    // 用户当前选中的其他节点（历史缺陷：测速结果张冠李戴）。
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

    let result = run_single_throughput_test_with_url(&node_tag, 5, mixed_port, &test_url, parallel_updown).await;

    // 持久化单节点测速历史
    if let Ok(ref res) = result {
        crate::core::stats_db::add_speedtest_record(&node_tag, res.download_bps, res.upload_bps, None);
    }

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

/// 查询节点测速历史（SQLite 持久化，时间倒序）
#[tauri::command]
pub async fn speedtest_get_history(
    node_tag: String,
    limit: Option<u32>,
) -> Result<ApiResponse<Vec<crate::core::stats_db::SpeedtestRecord>>, AppError> {
    let limit = limit.unwrap_or(20);
    match crate::core::stats_db::get_speedtest_history(&node_tag, limit) {
        Ok(records) => Ok(ApiResponse::ok(records)),
        Err(e) => Ok(ApiResponse::err(format!("查询测速历史失败: {}", e), 500)),
    }
}
