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

/// 触发延迟测速（调用 ClashAPI /proxies/{tag}/delay 触发测试，使用 JoinSet 并发提速）
///
/// 返回 HashMap<String, u16>：
/// - delay > 0：测速成功，值为延迟毫秒数
/// - delay = 0：测速失败（超时或不可达），前端可区分「已测试但失败」与「未测试」
#[tauri::command]
pub async fn speedtest_run_latency(
    _group_tag: String,
    node_tags: Vec<String>,
) -> Result<ApiResponse<HashMap<String, u16>>, AppError> {
    info!("触发共 {} 个节点的异步并发延迟测试", node_tags.len());
    let clash_client = Arc::new(ClashApiClient::default());
    let mut join_set = tokio::task::JoinSet::new();

    for tag in node_tags {
        let client = clash_client.clone();
        join_set.spawn(async move {
            match client.get_node_delay(&tag, "https://www.gstatic.com/generate_204", 5000).await {
                Ok(delay) => (tag, delay),
                Err(e) => {
                    log::warn!("[speedtest] 节点 [{}] 延迟测试失败: {}", tag, e);
                    // 返回 0 表示已测试但失败，前端可区分「未测试」与「超时」
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
    info!("开始对节点 [{}] 运行单体吞吐量测速...", node_tag);
    let port = crate::speedtest::get_mixed_port(&app_handle);
    match run_single_throughput_test(&node_tag, 5, port).await {
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
