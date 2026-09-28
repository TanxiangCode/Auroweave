/// IPC 命令 — 智能测速
/// 作者: TanXiang
use crate::core::clash_api::ClashApiClient;
use crate::core::test_core::{
    plan_batches, TestCoreManager, DEFAULT_PORT_BASE, TEST_CORE_BATCH_SIZE,
};
use crate::error::{ApiResponse, AppError};
use crate::speedtest::scheduler::SpeedTestScheduler;
use crate::speedtest::throughput::run_single_throughput_test_with_url;
use crate::speedtest::ThroughputResult;
use log::info;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};

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
    // 削峰限制：默认并发 20，上限 32，防止大批量随机子域并发轰炸触发公共 DNS 限流
    let concurrency = (settings.latency_test_concurrency as usize).clamp(1, 32);

    let timeout_ms = settings.latency_test_timeout_ms.clamp(500, 30000);
    let test_url = if settings.latency_test_url.trim().is_empty() {
        "http://www.gstatic.com/generate_204".to_string()
    } else {
        settings.latency_test_url.clone()
    };

    log::info!(
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

    // 防御：剔除「策略组」tag。组不是可测延迟的真实出口，测它会得到组当前
    // 所选节点的延迟，并被当成该节点的结果写库——实测历史库中出现过
    // `auto` / `SG-auto` 两条组 tag 记录（stats.dat delay_ms=0），
    // 污染节点存活趋势并让后续统计失真。
    // 前端已按 type 过滤，这里是后端兜底（单节点测速入口无此前端过滤）。
    let node_tags = match filter_out_group_tags(&clash_client, node_tags).await {
        Ok(v) => v,
        Err(e) => {
            // 读不到类型时保守放行：内核可能尚未就绪，不该因此让整个测速不可用
            log::warn!("[speedtest] 读取策略组类型失败，跳过组 tag 过滤: {}", e);
            return Ok(ApiResponse::err(
                AppError::Network(format!("无法校验节点列表（策略组类型读取失败）: {}", e)),
                502,
            ));
        }
    };
    if node_tags.is_empty() {
        return Ok(ApiResponse::err(
            AppError::Validation("节点列表中不含可测试的真实节点".to_string()),
            400,
        ));
    }

    // 重置取消标志
    LATENCY_CANCELLED.store(false, Ordering::Relaxed);

    // 测速前轻量刷新本地系统 DNS 缓存（环境自洁，杜绝旧客户端 Fake-IP 残留导致 15 秒假死）
    crate::system::sysproxy::flush_system_dns_cache();

    // 统一延迟模式走 test-core + 持久连接二次请求；关闭时继续使用 ClashAPI /delay。
    if settings.latency_unified_delay {
        let results = run_unified_latency_test(
            app_handle.clone(),
            group_tag,
            node_tags,
            concurrency,
            timeout_ms,
            test_url,
            settings.latency_persistent_reuse,
        )
        .await;
        return Ok(ApiResponse::ok(results));
    }

    let total = node_tags.len();
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

            let first = client.get_node_delay(&tag, &url, timeout_ms).await;
            let mut delay = match &first {
                Ok(d) => *d,
                Err(e) => {
                    log::debug!("[speedtest] 节点 [{}] 延迟测试失败: {}", tag, e);
                    0u16
                }
            };

            // 仅对「值不值得重试」的失败做一次重试。
            //
            // 旧实现对**所有**失败统一「退避 400ms 重试 1 次」。实测该订阅延迟
            // p50=803ms / p90=1437ms / max=2975ms，超时预算 3000ms——超时的节点
            // 重试仍然超时，这 400ms + 3s 纯属把整批耗时翻倍：
            // 370 节点 ÷ 并发 20 = 19 波 × (3s+0.4s+3s) ≈ 121s 最坏耗时。
            //
            // 现改为按失败分类决策：只有传输层瞬态错误值得立刻重试；
            // 超时/不可达/DNS 属确定性失败，直接判失败——探测面会按分类退避后
            // 在独立进程上重试，既不占用户带宽也不拖慢本批。
            if delay == 0 && !LATENCY_CANCELLED.load(Ordering::Relaxed) {
                let transient = match &first {
                    Err(e) => is_transient_delay_error(e),
                    Ok(_) => false,
                };
                if transient {
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

    // 与测速/解锁统一使用空节点终止哨兵。前端不能只依赖 invoke Promise 的 finally：
    // 大批量任务若前端保护性超时，迟到的进度事件仍会到达，必须有后端权威终止事件收尾。
    if let Err(e) = app_handle.emit(
        "latency-test-progress",
        LatencyProgressPayload {
            current_index: total,
            total,
            current_node: String::new(),
            delay: 0,
        },
    ) {
        log::warn!("[speedtest] 发送延迟测试终止事件失败: {}", e);
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

    // 不再触发内核 URLTest 组级重选。
    //
    // 旧实现在前端刚探完 370 个节点之后，又让内核对该组再全量探一次
    // （trigger_urltest_group_delay），与 180s 周期心跳撞在一起，等于把探测量
    // 翻倍；而选点职责现已归探测面（唯一真相源 + 独立进程），内核侧重复测量
    // 只会重新把探测流量压回主实例——正是 15s 超时的成因。
    //
    // 同时把本批结果灌回探测表：用户手动点「测延迟」是一次高意图全量测量，
    // 不应让这些结果只停留在 speedtest_history 而与调度器状态脱节。
    let probe_state: Option<std::sync::Arc<crate::probe::scheduler::ProbeScheduler>> =
        app_handle.try_state::<std::sync::Arc<crate::probe::scheduler::ProbeScheduler>>().map(|s| s.inner().clone());
    if let Some(sch) = probe_state {
        let table = sch.table();
        let snapshot: Vec<(String, u16)> = results
            .iter()
            .map(|(t, d)| (t.clone(), *d))
            .collect();
        tauri::async_runtime::spawn(async move {
            let mut t = table.write().await;
            let now = chrono::Utc::now().timestamp_millis();
            for (tag, d) in snapshot {
                if let Some(n) = t.get_mut(&tag) {
                    if d > 0 {
                        n.record_success(d, now);
                    } else {
                        n.record_failure(
                            crate::probe::table::FailureClass::Timeout,
                            now,
                        );
                    }
                }
            }
        });
    }

    log::info!("延迟测试完成: 成功 {} / 失败 {} / 总计 {}",
        results.values().filter(|&&d| d > 0).count(),
        results.values().filter(|&&d| d == 0).count(),
        results.len()
    );

    Ok(ApiResponse::ok(results))
}

/// 剔除列表中的「策略组」tag，只保留真实节点
///
/// 组（selector / urltest / fallback）不是可测延迟的出口：对它取 delay 得到的是
/// **组当前所选节点**的延迟，写库后会被当成该节点自己的结果。实测历史库中因此
/// 出现过 `auto` / `SG-auto` 两条组 tag 记录（delay_ms=0）。
///
/// 内核不可达时返回 Err，由调用方决定策略（当前实现选择明确失败而非静默放行，
/// 因为放行等于把「未知是否为组」当作「是节点」处理，正是本函数要防的事）。
async fn filter_out_group_tags(
    client: &ClashApiClient,
    tags: Vec<String>,
) -> Result<Vec<String>, AppError> {
    let json = client.get_proxies().await?;
    let proxies = json
        .get("proxies")
        .and_then(|p| p.as_object())
        .ok_or_else(|| AppError::Network("ClashAPI /proxies 响应缺少 proxies 字段".into()))?;

    // 读不到类型的 tag 一律剔除：宁可少测，也不能把组当节点测
    let mut kept = Vec::with_capacity(tags.len());
    let mut dropped = Vec::new();
    for tag in tags {
        match proxies.get(&tag).and_then(|v| v.get("type")).and_then(|t| t.as_str()) {
            Some(t) if is_group_type(t) => dropped.push(tag),
            Some(_) => kept.push(tag),
            None => dropped.push(tag),
        }
    }
    if !dropped.is_empty() {
        log::info!(
            "[speedtest] 剔除 {} 个非节点 tag（策略组或已下线）: {:?}",
            dropped.len(),
            &dropped[..dropped.len().min(5)]
        );
    }
    Ok(kept)
}

/// sing-box ClashAPI 报告的策略组类型（不是可测延迟的真实出口）
fn is_group_type(t: &str) -> bool {
    matches!(
        t.to_ascii_lowercase().as_str(),
        "selector" | "urltest" | "fallback" | "loadbalance"
            | "direct" | "block" | "reject" | "dns"
    )
}

/// 判定延迟测试失败是否为「传输层瞬态错误」（值得立即重试一次）
///
/// 与探测面 `FailureClass::worth_retry_now` 保持同一判据：只有 TLS 握手类
/// 瞬态错误值得重试；超时 / 不可达 / DNS 失败都是确定性结果，重试只会让
/// 整批耗时翻倍（实测 370 节点最坏 121s）。
fn is_transient_delay_error(e: &AppError) -> bool {
    let AppError::Network(msg) = e else {
        return false;
    };
    let m = msg.to_lowercase();
    // 504 = 内核侧探测超时（确定性：节点就是慢/死）
    // 503 = 不可达（确定性）
    // 握手/证书类瞬态错误才重试
    !m.contains("504")
        && !m.contains("503")
        && !m.contains("超时")
        && !m.contains("timeout")
        && !m.contains("不可达")
        && (m.contains("tls")
            || m.contains("handshake")
            || m.contains("握手")
            || m.contains("证书")
            || m.contains("connection reset")
            || m.contains("reset by peer"))
}

fn build_unified_probe_client(port: u16, timeout_ms: u64) -> Option<reqwest::Client> {
    let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{}", port)).ok()?;
    reqwest::Client::builder()
        .proxy(proxy)
        .pool_idle_timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_millis(timeout_ms))
        .timeout(Duration::from_millis(timeout_ms + 1_500))
        .build()
        .ok()
}

async fn measure_unified_request(client: &reqwest::Client, test_url: &str) -> Option<u16> {
    let started = Instant::now();
    let response = client.get(test_url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.bytes().await.ok()?;
    Some(
        started
            .elapsed()
            .as_millis()
            .clamp(1, u16::MAX as u128) as u16,
    )
}

/// 统一延迟单节点探测：先预热；按配置复用同一连接或新建冷连接测量。
async fn measure_unified_probe_delay(
    port: u16,
    test_url: &str,
    timeout_ms: u64,
    persistent_reuse: bool,
) -> u16 {
    if LATENCY_CANCELLED.load(Ordering::Relaxed) {
        return 0;
    }
    let client = match build_unified_probe_client(port, timeout_ms) {
        Some(client) => client,
        None => return 0,
    };

    // 第一次请求负责建立并预热客户端/节点连接；必须完整消费响应才能可靠复用连接。
    let warmup = match client.get(test_url).send().await {
        Ok(response) if response.status().is_success() => response,
        _ => return 0,
    };
    if warmup.bytes().await.is_err() || LATENCY_CANCELLED.load(Ordering::Relaxed) {
        return 0;
    }

    if persistent_reuse {
        return measure_unified_request(&client, test_url).await.unwrap_or(0);
    }

    // 高级对照模式：丢弃预热客户端，以全新连接测第二次 RTT。
    drop(client);
    let cold_client = match build_unified_probe_client(port, timeout_ms) {
        Some(client) => client,
        None => return 0,
    };
    measure_unified_request(&cold_client, test_url).await.unwrap_or(0)
}

fn emit_latency_terminal(app: &AppHandle, total: usize) {
    if let Err(e) = app.emit(
        "latency-test-progress",
        LatencyProgressPayload {
            current_index: total,
            total,
            current_node: String::new(),
            delay: 0,
        },
    ) {
        log::warn!("[speedtest] 发送延迟测试终止事件失败: {}", e);
    }
}

fn record_latency_result(
    app: &AppHandle,
    done_index: &mut usize,
    total: usize,
    results: &mut HashMap<String, u16>,
    pending: &mut Vec<(String, u16)>,
    tag: String,
    delay: u16,
) {
    *done_index += 1;
    let _ = app.emit(
        "latency-test-progress",
        LatencyProgressPayload {
            current_index: (*done_index).min(total),
            total,
            current_node: tag.clone(),
            delay,
        },
    );
    results.insert(tag.clone(), delay);
    pending.push((tag, delay));
}

/// test-core 统一延迟全量流程：每批一组，复用同一 HTTP 客户端连接测二次 RTT。
///
/// 注：`group_tag` 已不再用于触发内核组级重选（探测流量不得回到主实例），
/// 保留参数是为了不改动调用方签名与既有日志语义。
async fn run_unified_latency_test(
    app: AppHandle,
    #[allow(unused_variables)] group_tag: String,
    node_tags: Vec<String>,
    concurrency: usize,
    timeout_ms: u64,
    test_url: String,
    persistent_reuse: bool,
) -> HashMap<String, u16> {
    let total = node_tags.len();
    let mut results = HashMap::new();
    let mut pending_records = Vec::new();
    let mut done_index = 0usize;

    // 与测速/解锁共享 test-core 全局锁，覆盖配置写入、子进程启动和停止的完整生命周期。
    let _core_guard = crate::core::test_core::acquire_global_lock().await;
    if LATENCY_CANCELLED.load(Ordering::Relaxed) {
        for tag in &node_tags {
            record_latency_result(&app, &mut done_index, total, &mut results, &mut pending_records, tag.clone(), 0);
        }
        emit_latency_terminal(&app, total);
        return results;
    }

    let pool = crate::commands::subscription::collect_active_outbounds().unwrap_or_default();
    let resolved: Vec<Option<crate::core::parser::ParsedOutbound>> = node_tags
        .iter()
        .map(|tag| {
            pool.iter()
                .find(|node| &node.tag == tag && crate::core::parser::is_valid_proxy_node(node))
                .cloned()
        })
        .collect();
    let settings = crate::commands::settings::settings_get_internal(&app);
    let port_base = if settings.test_core_port_base > 0 { settings.test_core_port_base } else { DEFAULT_PORT_BASE };
    let core_manager = TestCoreManager::new();
    log::info!(
        "[speedtest] 开始 unified-delay 测试 (节点: {}, 并发: {}, 批大小: {})",
        total, concurrency, TEST_CORE_BATCH_SIZE
    );

    for batch_indices in plan_batches(total, TEST_CORE_BATCH_SIZE) {
        if LATENCY_CANCELLED.load(Ordering::Relaxed) { break; }
        let valid: Vec<(String, crate::core::parser::ParsedOutbound)> = batch_indices
            .iter()
            .filter_map(|&index| resolved[index].as_ref().map(|node| (node_tags[index].clone(), node.clone())))
            .collect();
        let valid_nodes: Vec<crate::core::parser::ParsedOutbound> = valid.iter().map(|(_, node)| node.clone()).collect();
        if valid_nodes.is_empty() {
            for &index in &batch_indices {
                record_latency_result(&app, &mut done_index, total, &mut results, &mut pending_records, node_tags[index].clone(), 0);
            }
            continue;
        }

        let base = match core_manager.spawn(&valid_nodes, port_base).await {
            Ok(base) => base,
            Err(e) => {
                log::warn!("[speedtest] unified-delay test-core 拉起失败: {}", e);
                for (tag, _) in valid {
                    record_latency_result(&app, &mut done_index, total, &mut results, &mut pending_records, tag, 0);
                }
                continue;
            }
        };
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
        let mut join_set = tokio::task::JoinSet::new();
        for (port_seq, (tag, _)) in valid.into_iter().enumerate() {
            let sem = semaphore.clone();
            let url = test_url.clone();
            join_set.spawn(async move {
                let _permit = sem.acquire().await.ok();
                let delay =
                    measure_unified_probe_delay(base + port_seq as u16, &url, timeout_ms, persistent_reuse)
                        .await;
                (tag, delay)
            });
        }
        while let Some(joined) = join_set.join_next().await {
            if let Ok((tag, delay)) = joined {
                record_latency_result(
                    &app,
                    &mut done_index,
                    total,
                    &mut results,
                    &mut pending_records,
                    tag,
                    delay,
                );
            }
        }
        core_manager.stop().await;
    }
    // 取消或异常未覆盖的节点补 0，并保证进度/终止事件完整收尾。
    for tag in &node_tags {
        if !results.contains_key(tag) {
            record_latency_result(
                &app,
                &mut done_index,
                total,
                &mut results,
                &mut pending_records,
                tag.clone(),
                0,
            );
        }
    }
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
    emit_latency_terminal(&app, total);

    // 同上：不再触发内核 URLTest 组级重选（探测流量不得回到主实例）
    log::info!(
        "[speedtest] unified-delay 完成: 成功 {} / 失败 {} / 总计 {}",
        results.values().filter(|&&delay| delay > 0).count(),
        results.values().filter(|&&delay| delay == 0).count(),
        results.len()
    );
    results
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
    // 用户选择的下载数据源（设置页 speed_test_url；支持预设回退与自定义地址）
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
        // 单节点也必须持有 test-core 全局锁：所有实例共享 config_test.json。
        let _core_guard = crate::core::test_core::acquire_global_lock().await;
        // 单节点错开 +500 端口段；全局锁负责配置/进程互斥。
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
    // 内存缓存是本会话最新值；启动/重启时再用 SQLite 最近记录补齐。
    let mut results = scheduler.get_results();
    match crate::core::stats_db::get_latest_speedtest_per_node() {
        Ok(persisted) => {
            for (tag, record) in persisted {
                results.entry(tag).or_insert_with(|| ThroughputResult {
                    download_bps: record.download_bps,
                    upload_bps: record.upload_bps,
                    tested_at: record.tested_at,
                });
            }
        }
        Err(e) => log::warn!("[speedtest] 读取持久化测速结果失败: {}", e),
    }
    Ok(ApiResponse::ok(results))
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

#[cfg(test)]
mod tests {
    #[test]
    fn group_type_detection_covers_all_sing_box_group_kinds() {
        // 策略组一律不可测延迟：对它取 delay 得到的是「组当前所选节点」的延迟，
        // 写库后会被当成该节点的结果（实测历史库出现过 auto / SG-auto 两条）
        for t in ["Selector", "URLTest", "Fallback", "LoadBalance", "Direct", "Block", "Reject", "DNS"] {
            assert!(is_group_type(t), "{} 应被识别为组", t);
        }
        // 真实节点类型不得被误判为组
        for t in ["AnyTLS", "Hysteria2", "Shadowsocks", "Trojan", "VMess", "VLESS", "TUIC"] {
            assert!(!is_group_type(t), "{} 是真实节点，不应被剔除", t);
        }
        // 大小写不敏感（ClashAPI 返回大小写不统一）
        assert!(is_group_type("selector") && is_group_type("URLTEST"));
    }

    use super::*;
    use crate::core::test_core::single_node_port_base;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn unified_probe_reuses_one_proxy_connection() {
        LATENCY_CANCELLED.store(false, Ordering::Relaxed);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let connections = Arc::new(AtomicUsize::new(0));
        let server_connections = connections.clone();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            server_connections.fetch_add(1, Ordering::SeqCst);
            let mut buffer = vec![0u8; 4096];
            loop {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 { break; }
                socket.write_all(b"HTTP/1.1 204 No Content\r\nConnection: keep-alive\r\n\r\n").await.unwrap();
            }
        });

        let delay =
            measure_unified_probe_delay(port, "http://www.gstatic.com/generate_204", 3_000, true)
                .await;
        server.abort();
        assert!(delay > 0);
        assert_eq!(connections.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unified_probe_cold_mode_uses_new_connection() {
        LATENCY_CANCELLED.store(false, Ordering::Relaxed);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let connections = Arc::new(AtomicUsize::new(0));
        let server_connections = connections.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                server_connections.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut buffer = vec![0u8; 4096];
                    loop {
                        let read = socket.read(&mut buffer).await.unwrap_or(0);
                        if read == 0 { break; }
                        let _ = socket.write_all(
                            b"HTTP/1.1 204 No Content\r\nConnection: keep-alive\r\n\r\n",
                        ).await;
                    }
                });
            }
        });

        let delay =
            measure_unified_probe_delay(port, "http://www.gstatic.com/generate_204", 3_000, false)
                .await;
        server.abort();
        assert!(delay > 0);
        assert_eq!(connections.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    #[ignore = "requires a current subscription and network access"]
    async fn live_unified_probe_current_subscription() {
        LATENCY_CANCELLED.store(false, Ordering::Relaxed);
        let nodes: Vec<_> = crate::commands::subscription::collect_active_outbounds()
            .unwrap_or_default()
            .into_iter()
            .filter(|node| crate::core::parser::is_valid_proxy_node(node))
            .take(16)
            .collect();
        assert!(!nodes.is_empty(), "current subscription has no valid node");
        let _guard = crate::core::test_core::acquire_global_lock().await;
        let core = TestCoreManager::new();
        let base = core
            .spawn(&nodes, single_node_port_base(DEFAULT_PORT_BASE))
            .await
            .unwrap();
        let mut tasks = tokio::task::JoinSet::new();
        for (index, _) in nodes.into_iter().enumerate() {
            tasks.spawn(async move {
                measure_unified_probe_delay(
                    base + index as u16,
                    "http://www.gstatic.com/generate_204",
                    5_000,
                    true,
                )
                .await
            });
        }
        let mut delays = Vec::new();
        while let Some(joined) = tasks.join_next().await {
            if let Ok(delay) = joined { delays.push(delay); }
        }
        core.stop().await;
        let success = delays.iter().filter(|&&delay| delay > 0).count();
        println!("live unified delay: {}/{} nodes succeeded", success, delays.len());
        assert!(success > 0);
    }
}
