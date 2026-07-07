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

/// 触发延迟测速（调用 ClashAPI /proxies/{tag}/delay 触发测试）
#[tauri::command]
pub async fn speedtest_run_latency(
    _group_tag: String,
    node_tags: Vec<String>,
) -> Result<ApiResponse<HashMap<String, u16>>, AppError> {
    info!("触发共 {} 个节点的延迟测试", node_tags.len());
    let clash_client = ClashApiClient::default();
    let mut results = HashMap::new();

    for tag in node_tags {
        if let Ok(delay) = clash_client.get_node_delay(&tag, "https://www.gstatic.com/generate_204", 5000).await {
            results.insert(tag, delay);
        }
    }

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
