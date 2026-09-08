/// 串行批量测速调度器
/// 作者: TanXiang
///
/// 双路径（plans/plan-N-test-core.md N-5）：
/// - 主路径 test-core：独立测试内核实例，节点池快照 + 专属端口，selector/用户流量
///   零打扰；并发受 settings.speedtest_test_concurrency 控制（默认 1 = 串行，
///   并发会互相抢带宽致数值失真——迁移只为零打扰，不为提速）
/// - 降级路径：test-core 拉起失败走 v1 selector 轮换（原语义不变）
use super::{throughput::run_single_throughput_test_with_url, BatchProgress, ThroughputResult};
use crate::core::clash_api::ClashApiClient;
use crate::core::test_core::{plan_batches, TestCoreManager, DEFAULT_PORT_BASE, TEST_CORE_BATCH_SIZE};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::mpsc;
use tracing::{info, warn};

pub struct SpeedTestScheduler {
    cancel_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
    results_cache: Arc<Mutex<HashMap<String, ThroughputResult>>>,
}

impl SpeedTestScheduler {
    pub fn new() -> Self {
        Self {
            cancel_tx: Arc::new(Mutex::new(None)),
            results_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 批量测速入口：test-core 零打扰路径，失败降级 selector 轮换
    pub async fn run_batch(
        &self,
        app: tauri::AppHandle,
        group_tag: String,
        node_tags: Vec<String>,
    ) {
        // 取消上一轮任务，避免两轮并发互相污染
        self.cancel();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let (tx, mut rx) = mpsc::channel::<()>(1);
        *self.cancel_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

        let cache = self.results_cache.clone();
        let clash_client = ClashApiClient::default();
        let mixed_port = super::get_mixed_port(&app);
        // 用户设置的测速数据源（设置页 speed_test_url；空串回退内置 Cloudflare）
        let test_url = crate::commands::settings::settings_get_internal(&app).speed_test_url;
        let settings = crate::commands::settings::settings_get_internal(&app);
        let port_base = if settings.test_core_port_base > 0 {
            settings.test_core_port_base
        } else {
            DEFAULT_PORT_BASE
        };
        // 吞吐并发默认 1（串行）：并发抢带宽数值失真；上限 4 防误配拖垮全批
        let concurrency = (settings.speedtest_test_concurrency.max(1) as usize).min(4);
        // 上下行并行测速（O-6，默认关保精度）
        let parallel_updown = settings.speedtest_parallel_updown;

        tokio::spawn(async move {
            // 全局 test-core 互斥：与解锁检测调度器共享（并发 spawn 端口打架）
            let _core_guard = crate::core::test_core::acquire_global_lock().await;

            let total = node_tags.len();
            info!(
                "开始批量测速 (共 {} 个节点, test-core 路径, 并发: {})...",
                total, concurrency
            );

            // 节点池快照：test-core 消费生成时刻的订阅数据
            let pool = crate::commands::subscription::collect_active_outbounds()
                .unwrap_or_default();
            let mut resolved: Vec<(String, Option<crate::core::parser::ParsedOutbound>)> =
                Vec::with_capacity(total);
            for tag in &node_tags {
                let node = pool
                    .iter()
                    .find(|n| n.tag == *tag)
                    .filter(|n| crate::core::parser::is_valid_proxy_node(n))
                    .cloned();
                if node.is_none() {
                    warn!("[speedtest] 节点 [{}] 不在订阅池（刷新后消失），记 0 结果", tag);
                }
                resolved.push((tag.clone(), node));
            }

            let mut done_index: usize = 0;
            let mut cancelled = false;
            let core_manager = TestCoreManager::new();

            'batch_loop: for batch_indices in
                plan_batches(resolved.len(), TEST_CORE_BATCH_SIZE)
            {
                if rx.try_recv().is_ok() {
                    cancelled = true;
                    break;
                }
                let batch: Vec<(String, Option<crate::core::parser::ParsedOutbound>)> =
                    batch_indices.iter().map(|&i| resolved[i].clone()).collect();
                let valid_nodes: Vec<crate::core::parser::ParsedOutbound> =
                    batch.iter().filter_map(|(_, n)| n.clone()).collect();

                let base = match core_manager.spawn(&valid_nodes, port_base).await {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("[speedtest] test-core 拉起失败，批量测速降级 selector 轮换: {}", e);
                        break 'batch_loop; // 未完成节点走 legacy
                    }
                };

                // 批内受控并发测速；JoinSet 任务取 owned（端口/节点名/配置克隆）
                let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
                let mut join_set = tokio::task::JoinSet::new();
                let mut port_seq = 0usize;
                for (tag, node) in &batch {
                    if node.is_none() {
                        continue; // 无效节点不入 test-core
                    }
                    let port = base + port_seq as u16;
                    port_seq += 1;
                    join_set.spawn({
                        let sem = semaphore.clone();
                        let tag = tag.clone();
                        let test_url = test_url.clone();
                        let parallel_updown = parallel_updown;
                        async move {
                            let _permit = sem.acquire().await.ok();
                            let res = run_single_throughput_test_with_url(
                                &tag,
                                3,
                                port,
                                &test_url,
                                parallel_updown,
                            )
                            .await;
                            let r = res.unwrap_or(ThroughputResult {
                                download_bps: 0,
                                upload_bps: 0,
                                tested_at: chrono::Utc::now().timestamp_millis(),
                            });
                            (tag, r)
                        }
                    });
                }

                while let Some(joined) = join_set.join_next().await {
                    if rx.try_recv().is_ok() {
                        cancelled = true;
                        break; // 在飞测速自然收尾后统一退出
                    }
                    if let Ok((tag, res)) = joined {
                        cache
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(tag.clone(), res.clone());
                        done_index += 1;
                        let _ = app.emit(
                            "speedtest-progress",
                            BatchProgress {
                                current_index: done_index.min(total),
                                total,
                                current_node: tag.clone(),
                                result: Some(res.clone()),
                            },
                        );
                        persist_one((tag, res));
                    }
                }
                core_manager.stop().await;
            }

            // ---- 降级/收尾：未完成节点走 selector 轮换（v1 语义） ----
            if !cancelled {
                let tested: std::collections::HashSet<String> = cache
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .keys()
                    .cloned()
                    .collect();
                let remaining: Vec<String> = node_tags
                    .iter()
                    .filter(|t| !tested.contains(*t))
                    .cloned()
                    .collect();
                if !remaining.is_empty() {
                    warn!("[speedtest] {} 个节点走 selector 轮换路径", remaining.len());
                    run_selector_loop(
                        &app,
                        &clash_client,
                        &group_tag,
                        mixed_port,
                        &test_url,
                        remaining,
                        &mut rx,
                        total,
                        &mut done_index,
                        &cache,
                        parallel_updown,
                    )
                    .await;
                }
            }

            // 终止事件：无论正常结束还是取消，均以 current_index == total 通知前端复位状态
            let _ = app.emit(
                "speedtest-progress",
                BatchProgress {
                    current_index: total,
                    total,
                    current_node: String::new(),
                    result: None,
                },
            );

            info!("批量测速流程结束");
        });
    }

    /// 取消当前正在运行的批量测速
    pub fn cancel(&self) {
        if let Some(tx) = self.cancel_tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.try_send(());
        }
    }

    /// 获取缓存结果
    pub fn get_results(&self) -> HashMap<String, ThroughputResult> {
        self.results_cache.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// 单条测速记录落库（spawn_blocking 避免 std Mutex + fsync 阻塞 async worker）
fn persist_one(rec: (String, ThroughputResult)) {
    let (tag, res) = rec;
    let (dl, ul) = (res.download_bps, res.upload_bps);
    tokio::task::spawn_blocking(move || {
        crate::core::stats_db::add_speedtest_record(&tag, dl, ul, None);
    });
}

/// selector 轮换降级路径（v1 语义：切组 → 测速 → 还原）
async fn run_selector_loop(
    app: &tauri::AppHandle,
    clash_client: &ClashApiClient,
    group_tag: &str,
    mixed_port: u16,
    test_url: &str,
    node_tags: Vec<String>,
    rx: &mut mpsc::Receiver<()>,
    total: usize,
    done_index: &mut usize,
    cache: &Arc<Mutex<HashMap<String, ThroughputResult>>>,
    parallel_updown: bool,
) {
    let original_now: Option<String> = clash_client.get_proxies().await.ok()
        .and_then(|json| json.get("proxies")?.get(&group_tag)?.get("now")?.as_str().map(|s| s.to_string()));
    let original_now = original_now.filter(|s| !s.is_empty());

    for node_tag in node_tags {
        if rx.try_recv().is_ok() {
            warn!("批量测速已由用户取消");
            break;
        }

        // 1. 切换选择器节点。失败时跳过该节点（记录 0 结果），
        //    避免把当前节点（上一个节点）的测速结果错误归属到目标节点名下
        if let Err(e) = clash_client.select_node(group_tag, &node_tag).await {
            warn!("[speedtest] 切换节点 [{}] 失败，跳过测速: {}", node_tag, e);
            let res = ThroughputResult {
                download_bps: 0,
                upload_bps: 0,
                tested_at: chrono::Utc::now().timestamp_millis(),
            };
            cache.lock().unwrap_or_else(|e| e.into_inner()).insert(node_tag.clone(), res.clone());
            *done_index += 1;
            let _ = app.emit(
                "speedtest-progress",
                BatchProgress {
                    current_index: (*done_index).min(total),
                    total,
                    current_node: node_tag,
                    result: Some(res),
                },
            );
            continue;
        }

        // 2. 测量 3 秒速度（用户设置的测速 URL，经主 mixed 端口）
        let res = run_single_throughput_test_with_url(&node_tag, 3, mixed_port, test_url, parallel_updown)
            .await
            .unwrap_or(ThroughputResult {
                download_bps: 0,
                upload_bps: 0,
                tested_at: chrono::Utc::now().timestamp_millis(),
            });

        // 3. 更新缓存 + 落库
        cache.lock().unwrap_or_else(|e| e.into_inner()).insert(node_tag.clone(), res.clone());
        persist_one((node_tag.clone(), res.clone()));
        *done_index += 1;

        let _ = app.emit(
            "speedtest-progress",
            BatchProgress {
                current_index: (*done_index).min(total),
                total,
                current_node: node_tag,
                result: Some(res),
            },
        );
    }

    // 还原用户原选中节点
    if let Some(orig) = original_now {
        if let Err(e) = clash_client.select_node(group_tag, &orig).await {
            warn!("批量测速后还原节点选择失败: {}", e);
        }
    }
}

impl Default for SpeedTestScheduler {
    fn default() -> Self {
        Self::new()
    }
}
