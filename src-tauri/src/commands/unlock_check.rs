/// IPC 命令 — AI 服务解锁检测（Gemini / Claude / ChatGPT）
/// 作者: TanXiang
///
/// 架构（plans/plan-N-test-core.md）：
/// - **主路径（test-core）**：拉起独立测试内核实例，N 入站/N 出站/inbound 规则
///   钉死——每节点一个专属端口，Semaphore 受控并发检测，主 selector/用户流量零打扰。
/// - **降级路径（selector 轮换）**：test-core 拉起失败时逐节点切主 selector 检测
///   （v1 语义，batch 串行）。
/// ip 层随批次并发：ip-api 限速按出口 IP 计数（45/min per IP），批量下每查询
/// 来自各节点自己的出口，天然分散；429 时单发换 freeipapi 容错。
use crate::core::clash_api::ClashApiClient;
use crate::core::test_core::{plan_batches, TestCoreManager, DEFAULT_PORT_BASE, TEST_CORE_BATCH_SIZE};
use crate::core::unlock_check::{
    check_current_exit, UnlockCheckParams, UnlockCheckResult, UnlockService, UnlockStatus,
};
use crate::error::{ApiResponse, AppError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::mpsc;
use log::{info, warn};

/// 批量解锁检测进度事件（与 speedtest-progress 同形态：result=None 表示开始、
/// Some 表示完成；终止事件 current_index == total 且 current_node 为空）
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnlockBatchProgress {
    pub current_index: usize,
    pub total: usize,
    pub current_node: String,
    pub result: Option<UnlockCheckResult>,
}

pub struct UnlockCheckScheduler {
    cancel_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

impl UnlockCheckScheduler {
    pub fn new() -> Self {
        Self {
            cancel_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// 批量解锁检测入口：优先 test-core 零打扰路径，失败降级 selector 轮换
    pub async fn run_batch(
        &self,
        app: tauri::AppHandle,
        group_tag: String,
        node_tags: Vec<String>,
        services: Vec<UnlockService>,
        with_ip: bool,
    ) {
        // 与批量测速相同的互斥语义：两轮并发会互相污染
        self.cancel();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let (tx, mut rx) = mpsc::channel::<()>(1);
        *self.cancel_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

        let settings = crate::commands::settings::settings_get_internal(&app);
        let params = UnlockCheckParams::from_settings(&settings);
        let port_base = if settings.test_core_port_base > 0 {
            settings.test_core_port_base
        } else {
            DEFAULT_PORT_BASE
        };
        let concurrency = (settings.unlock_test_concurrency as usize).clamp(2, 16);
        let services: Vec<UnlockService> = if services.is_empty() {
            vec![UnlockService::Gemini, UnlockService::Claude, UnlockService::Chatgpt]
        } else {
            services
        };

        // 过滤公告/伪装节点（与延迟测速同款），过滤后再分批
        let node_tags: Vec<String> = node_tags
            .into_iter()
            .filter(|t| !crate::core::parser::is_announcement_or_fake_node(t, None, None))
            .collect();
        let total = node_tags.len();

        // 节点池：订阅存储快照（生成时刻锁定；订阅刷新不影响进行中批次）
        let pool = crate::commands::subscription::collect_active_outbounds()
            .unwrap_or_default();

        let core_manager = Arc::new(TestCoreManager::new());
        let clash_client = ClashApiClient::default();
        let mixed_port = crate::speedtest::get_mixed_port(&app);

        tokio::spawn(async move {
            // 全局 test-core 互斥：与吞吐测速调度器共享（并发 spawn 端口打架）
            let _core_guard = crate::core::test_core::acquire_global_lock().await;
            info!(
                "开始批量解锁检测 (共 {} 节点, 服务: {:?}, IP 层: {}, 并发: {})",
                total,
                services.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                with_ip,
                concurrency
            );

            let mut persist_batch: Vec<UnlockCheckResult> = Vec::with_capacity(total);
            let mut done_index: usize = 0;
            let mut cancelled = false;

            // tag → 订阅池节点匹配（未命中=订阅刚刷新节点已消失，记 Failed）
            let nodes_for_tags: Vec<(String, Option<crate::core::parser::ParsedOutbound>)> =
                node_tags
                    .iter()
                    .map(|tag| {
                        let node = pool
                            .iter()
                            .find(|n| n.tag == *tag)
                            .filter(|n| crate::core::parser::is_valid_proxy_node(n))
                            .cloned();
                        (tag.clone(), node)
                    })
                    .collect();

            // ---- test-core 主路径：分批（32/批）并发 ----
            for batch_indices in plan_batches(nodes_for_tags.len(), TEST_CORE_BATCH_SIZE) {
                if cancelled || rx.try_recv().is_ok() {
                    cancelled = true;
                    break;
                }

                let batch: Vec<(String, Option<crate::core::parser::ParsedOutbound>)> =
                    batch_indices.iter().map(|&i| nodes_for_tags[i].clone()).collect();
                let valid_nodes: Vec<crate::core::parser::ParsedOutbound> =
                    batch.iter().filter_map(|(_, n)| n.clone()).collect();

                // 全失效批次（节点均已被订阅刷新移除）：直接记 Failed，不起进程
                if valid_nodes.is_empty() {
                    for (tag, _) in &batch {
                        persist_batch.push(failed_result(tag, &services));
                        done_index += 1;
                        let _ = app.emit(
                            "unlock-check-progress",
                            UnlockBatchProgress {
                                current_index: done_index,
                                total,
                                current_node: tag.clone(),
                                result: Some(persist_batch.last().unwrap().clone()),
                            },
                        );
                    }
                    continue;
                }

                let spawn_res = core_manager.spawn(&valid_nodes, port_base).await;
                let base = match spawn_res {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("[unlock] test-core 拉起失败，整批降级 selector 轮换: {}", e);
                        break; // 跳出分批循环进入 legacy 路径（本批起）
                    }
                };

                // 批内并发探测：Semaphore 受控；端口 = base + 批内有效节点序号。
                // 任务取 owned 克隆（JoinSet 跨 await，不能借用外层局部）
                let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
                let mut join_set = tokio::task::JoinSet::new();
                let mut port_seq = 0usize;
                for (tag, node) in &batch {
                    if node.is_none() {
                        // 无效节点不入 test-core（端口序列只推进有效节点）
                        continue;
                    }
                    let port = base + port_seq as u16;
                    port_seq += 1;
                    join_set.spawn({
                        let sem = semaphore.clone();
                        let tag = tag.clone();
                        let services = services.clone();
                        let params = params.clone();
                        async move {
                            let _permit = sem.acquire().await.ok();
                            let res = check_current_exit(port, &services, with_ip, &params).await;
                            let mut r = res.unwrap_or_else(|_| failed_result(&tag, &services));
                            r.node_tag = tag;
                            r
                        }
                    });
                }

                let mut batch_results: Vec<(String, UnlockCheckResult)> = Vec::new();
                while let Some(joined) = join_set.join_next().await {
                    if rx.try_recv().is_ok() {
                        cancelled = true;
                        // 已提交的探测任务无法中断，等本批自然收尾后统一退出
                        break;
                    }
                    if let Ok(r) = joined {
                        batch_results.push((r.node_tag.clone(), r));
                    }
                }
                core_manager.stop().await;

                // 本批结果推送（进度跨批累计）
                for (tag, r) in batch_results {
                    persist_batch.push(r.clone());
                    done_index += 1;
                    let _ = app.emit(
                        "unlock-check-progress",
                        UnlockBatchProgress {
                            current_index: done_index.min(total),
                            total,
                            current_node: tag,
                            result: Some(r),
                        },
                    );
                }
            }

            // ---- 降级路径：从尚未完成的节点起走 selector 轮换（v1 语义） ----
            if !cancelled {
                let remaining: Vec<String> = node_tags
                    .iter()
                    .filter(|t| !persist_batch.iter().any(|r| &r.node_tag == *t))
                    .cloned()
                    .collect();
                if !remaining.is_empty() {
                    warn!("[unlock] {} 个节点走 selector 轮换降级路径", remaining.len());
                    let legacy = run_selector_loop(
                        &app,
                        &clash_client,
                        &group_tag,
                        mixed_port,
                        remaining,
                        &services,
                        with_ip,
                        &params,
                        &mut rx,
                        total,
                        &mut done_index,
                        &mut persist_batch,
                    )
                    .await;
                    cancelled = cancelled || legacy;
                }
            }

            finish_batch(&app, persist_batch, total, cancelled).await;
        });
    }

    pub fn cancel(&self) {
        if if let Some(tx) = self.cancel_tx.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.try_send(());
            true
        } else {
            false
        } {
            info!("已请求取消批量解锁检测");
        }
    }
}

/// 生成全 Failed 结果（节点消失/切组失败/探测异常的归属安全兜底）
fn failed_result(tag: &str, services: &[UnlockService]) -> UnlockCheckResult {
    let mut services_map = HashMap::new();
    for s in services {
        services_map.insert(s.as_str().to_string(), UnlockStatus::Failed);
    }
    UnlockCheckResult {
        node_tag: tag.to_string(),
        services: services_map,
        egress_ip: None,
        country_code: None,
        hosting: None,
        proxy_flag: None,
        isp: None,
        tested_at: chrono::Utc::now().timestamp_millis(),
    }
}

/// selector 轮换降级路径（v1 语义原样保留：切组 → 检测 → 还原，批内串行）
///
/// 返回 true 表示中途取消。
async fn run_selector_loop(
    app: &tauri::AppHandle,
    clash_client: &ClashApiClient,
    group_tag: &str,
    mixed_port: u16,
    node_tags: Vec<String>,
    services: &[UnlockService],
    with_ip: bool,
    params: &UnlockCheckParams,
    rx: &mut mpsc::Receiver<()>,
    total: usize,
    done_index: &mut usize,
    persist_batch: &mut Vec<UnlockCheckResult>,
) -> bool {
    let original_now: Option<String> = clash_client
        .get_proxies()
        .await
        .ok()
        .and_then(|json| {
            json.get("proxies")?
                .get(group_tag)?
                .get("now")?
                .as_str()
                .map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty());

    for node_tag in node_tags {
        if rx.try_recv().is_ok() {
            warn!("[unlock] 降级路径已由用户取消");
            if let Some(orig) = &original_now {
                let _ = clash_client.select_node(group_tag, orig).await;
            }
            return true;
        }

        let _ = app.emit(
            "unlock-check-progress",
            UnlockBatchProgress {
                current_index: (*done_index + 1).min(total),
                total,
                current_node: node_tag.clone(),
                result: None,
            },
        );

        let result = match clash_client.select_node(group_tag, &node_tag).await {
            Ok(()) => match check_current_exit(mixed_port, services, with_ip, params).await {
                Ok(mut r) => {
                    r.node_tag = node_tag.clone();
                    r
                }
                Err(e) => {
                    warn!("[unlock] 降级路径节点 [{}] 检测失败: {}", node_tag, e);
                    failed_result(&node_tag, services)
                }
            },
            Err(e) => {
                warn!("[unlock] 降级路径切换节点 [{}] 失败: {}", node_tag, e);
                failed_result(&node_tag, services)
            }
        };

        let _ = app.emit(
            "unlock-check-progress",
            UnlockBatchProgress {
                current_index: (*done_index + 1).min(total),
                total,
                current_node: node_tag.clone(),
                result: Some(result.clone()),
            },
        );
        persist_batch.push(result);
        *done_index += 1;
    }

    if let Some(orig) = &original_now {
        if let Err(e) = clash_client.select_node(group_tag, orig).await {
            warn!("[unlock] 降级路径还原节点选择失败: {}", e);
        }
    }
    false
}

/// 批次收尾：攒批落库 + 终止事件（与延迟测速攒批同模式）
async fn finish_batch(
    app: &tauri::AppHandle,
    persist_batch: Vec<UnlockCheckResult>,
    total: usize,
    cancelled: bool,
) {
    if !persist_batch.is_empty() {
        tokio::task::spawn_blocking(move || {
            crate::core::stats_db::add_unlock_records_batch(persist_batch);
        });
    }
    let _ = app.emit(
        "unlock-check-progress",
        UnlockBatchProgress {
            current_index: total,
            total,
            current_node: String::new(),
            result: None,
        },
    );
    info!(
        "批量解锁检测流程结束（{} 节点完成{}）",
        total,
        if cancelled { "，中途取消" } else { "" }
    );
}

/// 解析前端服务参数（容错：未知值直接拒绝，避免静默全测）
fn parse_services(raw: Vec<String>) -> Result<Vec<UnlockService>, AppError> {
    if raw.is_empty() {
        return Ok(vec![]); // 空 = 全测
    }
    let mut out = Vec::with_capacity(raw.len());
    for s in raw {
        match UnlockService::from_str_value(&s) {
            Some(v) => out.push(v),
            None => {
                return Err(AppError::Validation(format!("未知的检测服务: {}", s)));
            }
        }
    }
    Ok(out)
}

/// 单节点解锁检测（test-core 零打扰路径：节点在订阅池 → 专属端口检测；
/// 不在订阅池（自定义分组显示名等场景）→ 降级 selector 轮换语义）
#[tauri::command]
pub async fn unlock_check_single(
    app_handle: tauri::AppHandle,
    node_tag: String,
    services: Option<Vec<String>>,
    with_ip: Option<bool>,
) -> Result<ApiResponse<UnlockCheckResult>, AppError> {
    let node_tag = node_tag.trim().to_string();
    if node_tag.is_empty() {
        return Ok(ApiResponse::err(
            AppError::Validation("节点名称不能为空".to_string()),
            400,
        ));
    }
    let services = parse_services(services.unwrap_or_default())?;
    let services: Vec<UnlockService> = if services.is_empty() {
        vec![UnlockService::Gemini, UnlockService::Claude, UnlockService::Chatgpt]
    } else {
        services
    };
    let with_ip = with_ip.unwrap_or(true);

    let settings = crate::commands::settings::settings_get_internal(&app_handle);
    let params = UnlockCheckParams::from_settings(&settings);
    let port_base = if settings.test_core_port_base > 0 {
        settings.test_core_port_base
    } else {
        DEFAULT_PORT_BASE
    };

    // ---- 主路径：订阅池命中 → test-core 专属端口（零打扰） ----
    if let Some(node) = crate::commands::subscription::collect_active_outbounds()
        .unwrap_or_default()
        .into_iter()
        .find(|n| n.tag == node_tag && crate::core::parser::is_valid_proxy_node(n))
    {
        let core = crate::core::test_core::TestCoreManager::new();
        // 单节点也必须持有 test-core 全局锁：所有实例共享 config_test.json。
        let _core_guard = crate::core::test_core::acquire_global_lock().await;
        // 单节点错开 +500 端口段；全局锁负责配置/进程互斥。
        let result = match core
            .spawn(std::slice::from_ref(&node), crate::core::test_core::single_node_port_base(port_base))
            .await
        {
            Ok(base) => {
                let res = check_current_exit(base, &services, with_ip, &params).await;
                core.stop().await; // 检测完立即销毁（短生命周期语义）
                match res {
                    Ok(mut r) => {
                        r.node_tag = node_tag.clone();
                        r
                    }
                    Err(e) => {
                        warn!("[unlock] 单节点 test-core 检测失败: {}", e);
                        failed_result(&node_tag, &services)
                    }
                }
            }
            Err(e) => {
                warn!("[unlock] test-core 拉起失败，单节点降级 selector 路径: {}", e);
                return check_single_via_selector(
                    &app_handle,
                    &node_tag,
                    &services,
                    with_ip,
                    &params,
                )
                .await;
            }
        };

        // 落库（失败仅记日志，见 add_unlock_record）
        let rec = result.clone();
        tokio::task::spawn_blocking(move || {
            crate::core::stats_db::add_unlock_record(&rec);
        });
        info!("[unlock] 单节点检测完成（test-core）: [{}]", node_tag);
        return Ok(ApiResponse::ok(result));
    }

    // ---- 降级：节点不在订阅池（selector 组内显示名/已删节点）走 v1 语义 ----
    check_single_via_selector(&app_handle, &node_tag, &services, with_ip, &params).await
}

/// 单节点 selector 轮换路径（v1 语义保留：切组 → 检测 → 还原）
async fn check_single_via_selector(
    app_handle: &tauri::AppHandle,
    node_tag: &str,
    services: &[UnlockService],
    with_ip: bool,
    params: &UnlockCheckParams,
) -> Result<ApiResponse<UnlockCheckResult>, AppError> {
    let clash_client = ClashApiClient::default();
    let mixed_port = crate::speedtest::get_mixed_port(app_handle);

    let mut restored: Option<(String, String)> = None;
    if let Ok(proxies) = clash_client.get_proxies().await {
        if let Some(proxies) = proxies.get("proxies").and_then(|p| p.as_object()) {
            let group = proxies.iter().find(|(_, v)| {
                v.get("type").and_then(|t| t.as_str()) == Some("Selector")
                    && v.get("all").and_then(|a| a.as_array())
                        .map(|a| a.iter().any(|t| t.as_str() == Some(node_tag)))
                        .unwrap_or(false)
            });
            if let Some((g_tag, g_val)) = group {
                let original_now = g_val.get("now").and_then(|n| n.as_str()).unwrap_or("").to_string();
                if original_now != node_tag {
                    clash_client
                        .select_node(g_tag, node_tag)
                        .await
                        .map_err(|e| AppError::Network(format!("无法切换到目标节点进行检测: {}", e)))?;
                    restored = Some((g_tag.clone(), original_now));
                }
            } else {
                warn!("[unlock] 节点 [{}] 不在任何 Selector 组中，按当前出站检测", node_tag);
            }
        }
    }

    let mut result = check_current_exit(mixed_port, services, with_ip, params).await?;
    result.node_tag = node_tag.to_string();

    // 落库
    let rec = result.clone();
    tokio::task::spawn_blocking(move || {
        crate::core::stats_db::add_unlock_record(&rec);
    });

    // 还原
    if let Some((g_tag, original_now)) = restored {
        if !original_now.is_empty() {
            if let Err(e) = clash_client.select_node(&g_tag, &original_now).await {
                warn!("[unlock] 还原节点选择失败: {}", e);
            }
        }
    }

    info!("[unlock] 单节点检测完成（selector 路径）: [{}]", node_tag);
    Ok(ApiResponse::ok(result))
}

/// 批量解锁检测（串行调度）
#[tauri::command]
pub async fn unlock_check_batch(
    app_handle: tauri::AppHandle,
    scheduler: tauri::State<'_, Arc<UnlockCheckScheduler>>,
    group_tag: String,
    node_tags: Vec<String>,
    services: Option<Vec<String>>,
    with_ip: Option<bool>,
) -> Result<ApiResponse<()>, AppError> {
    if node_tags.is_empty() {
        return Ok(ApiResponse::err(
            AppError::Validation("节点列表为空，无法启动批量解锁检测".to_string()),
            400,
        ));
    }
    let services = parse_services(services.unwrap_or_default())?;
    let with_ip = with_ip.unwrap_or(true);
    info!(
        "启动批量解锁检测队列，分组: {}, 节点数: {}, IP 层: {}",
        group_tag,
        node_tags.len(),
        with_ip
    );
    scheduler
        .run_batch(app_handle, group_tag, node_tags, services, with_ip)
        .await;
    Ok(ApiResponse::ok(()))
}

/// 取消批量解锁检测
#[tauri::command]
pub async fn unlock_check_cancel(
    scheduler: tauri::State<'_, Arc<UnlockCheckScheduler>>,
) -> Result<ApiResponse<()>, AppError> {
    info!("取消正在运行的批量解锁检测任务");
    scheduler.cancel();
    Ok(ApiResponse::ok(()))
}

/// 每节点最近一次解锁检测结果（应用启动/视图激活时回填 store）
#[tauri::command]
pub async fn unlock_check_get_latest(
) -> Result<ApiResponse<Vec<crate::core::stats_db::UnlockRecord>>, AppError> {
    match crate::core::stats_db::get_latest_unlock_per_node() {
        Ok(records) => Ok(ApiResponse::ok(records)),
        Err(e) => Ok(ApiResponse::err(format!("查询解锁检测历史失败: {}", e), 500)),
    }
}
