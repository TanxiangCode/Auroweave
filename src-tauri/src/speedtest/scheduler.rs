/// 串行批量测速调度器
/// 作者: TanXiang
use super::{throughput::run_single_throughput_test_with_url, BatchProgress, ThroughputResult};
use crate::core::clash_api::ClashApiClient;
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

    /// 串行启动批量测速
    pub async fn run_batch(
        &self,
        app: tauri::AppHandle,
        group_tag: String,
        node_tags: Vec<String>,
    ) {
        // 取消上一轮任务，避免两轮并发切换同一 Selector 组导致结果互相污染
        self.cancel();
        // 等待一个调度周期，让上一轮任务感知取消信号后退出
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let (tx, mut rx) = mpsc::channel::<()>(1);
        *self.cancel_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

        let cache = self.results_cache.clone();
        let clash_client = ClashApiClient::default();
        let port = super::get_mixed_port(&app);
        // 用户设置的测速数据源（设置页 speed_test_url；空串回退内置 Cloudflare）
        let test_url = crate::commands::settings::settings_get_internal(&app).speed_test_url;

        tokio::spawn(async move {
            let total = node_tags.len();
            info!("开始对 [{}] 展开批量串行测速 (共 {} 个节点)...", group_tag, total);

            // 记录用户原选中节点，测速结束后还原（批量测速会逐节点切换 selector，
            // 不还原会停留到最后一个测试节点，且测速期间用户流量跟着轮换）
            let original_now: Option<String> = clash_client.get_proxies().await.ok()
                .and_then(|json| json.get("proxies")?.get(&group_tag)?.get("now")?.as_str().map(|s| s.to_string()));
            let original_now = original_now.filter(|s| !s.is_empty());

            for (index, node_tag) in node_tags.iter().enumerate() {
                // 检查取消信号
                if rx.try_recv().is_ok() {
                    warn!("批量测速已由用户取消");
                    break;
                }

                // 1. 切换选择器节点。失败时跳过该节点（记录 0 结果），
                //    避免把当前节点（上一个节点）的测速结果错误归属到目标节点名下
                if let Err(e) = clash_client.select_node(&group_tag, node_tag).await {
                    warn!("[speedtest] 切换节点 [{}] 失败，跳过测速: {}", node_tag, e);
                    let res = ThroughputResult {
                        download_bps: 0,
                        upload_bps: 0,
                        tested_at: chrono::Utc::now().timestamp_millis(),
                    };
                    cache.lock().unwrap_or_else(|e| e.into_inner()).insert(node_tag.clone(), res.clone());
                    let _ = app.emit(
                        "speedtest-progress",
                        BatchProgress {
                            current_index: index + 1,
                            total,
                            current_node: node_tag.clone(),
                            result: Some(res),
                        },
                    );
                    continue;
                }

                // 进度推送 (开始测速)
                let _ = app.emit(
                    "speedtest-progress",
                    BatchProgress {
                        current_index: index + 1,
                        total,
                        current_node: node_tag.clone(),
                        result: None,
                    },
                );

                // 2. 测量 3 秒速度（用户设置的测速 URL）
                let res = run_single_throughput_test_with_url(node_tag, 3, port, &test_url).await.unwrap_or(ThroughputResult {
                    download_bps: 0,
                    upload_bps: 0,
                    tested_at: chrono::Utc::now().timestamp_millis(),
                });

                // 3. 更新缓存
                cache.lock().unwrap_or_else(|e| e.into_inner()).insert(node_tag.clone(), res.clone());

                // 持久化测速历史（重启不丢，SpeedtestView 可查趋势对比）
                crate::core::stats_db::add_speedtest_record(node_tag, res.download_bps, res.upload_bps, None);

                // 进度推送 (测速完成)
                let _ = app.emit(
                    "speedtest-progress",
                    BatchProgress {
                        current_index: index + 1,
                        total,
                        current_node: node_tag.clone(),
                        result: Some(res),
                    },
                );
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

            // 还原用户原选中节点
            if let Some(orig) = original_now {
                if let Err(e) = clash_client.select_node(&group_tag, &orig).await {
                    warn!("批量测速后还原节点选择失败: {}", e);
                }
            }

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

impl Default for SpeedTestScheduler {
    fn default() -> Self {
        Self::new()
    }
}
