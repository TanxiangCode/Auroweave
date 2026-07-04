/// 串行批量测速调度器
/// 作者: TanXiang
use super::{throughput::run_single_throughput_test, BatchProgress, ThroughputResult};
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
        // 取消上一轮任务
        self.cancel();

        let (tx, mut rx) = mpsc::channel::<()>(1);
        *self.cancel_tx.lock().unwrap() = Some(tx);

        let cache = self.results_cache.clone();
        let clash_client = ClashApiClient::default();

        tokio::spawn(async move {
            let total = node_tags.len();
            info!("开始对 [{}] 展开批量串行测速 (共 {} 个节点)...", group_tag, total);

            for (index, node_tag) in node_tags.iter().enumerate() {
                // 检查取消信号
                if rx.try_recv().is_ok() {
                    warn!("批量测速已由用户取消");
                    break;
                }

                // 1. 切换选择器节点
                let _ = clash_client.select_node(&group_tag, node_tag).await;

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

                // 2. 测量 3 秒速度
                let res = run_single_throughput_test(node_tag, 3).await.unwrap_or(ThroughputResult {
                    download_bps: 0,
                    upload_bps: 0,
                    tested_at: chrono::Utc::now().timestamp_millis(),
                });

                // 3. 更新缓存
                cache.lock().unwrap().insert(node_tag.clone(), res.clone());

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

            info!("批量测速流程结束");
        });
    }

    /// 取消当前正在运行的批量测速
    pub fn cancel(&self) {
        if let Some(tx) = self.cancel_tx.lock().unwrap().take() {
            let _ = tx.try_send(());
        }
    }

    /// 获取缓存结果
    pub fn get_results(&self) -> HashMap<String, ThroughputResult> {
        self.results_cache.lock().unwrap().clone()
    }
}

impl Default for SpeedTestScheduler {
    fn default() -> Self {
        Self::new()
    }
}
