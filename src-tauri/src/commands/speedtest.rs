/// IPC 命令 — 测速
/// 作者: TanXiang
use crate::error::ApiResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThroughputResult {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub tested_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpeedTestTask {
    pub node_tag: String,
    pub status: String, // idle | testing_latency | testing_speed | done | error
    pub progress: Option<u8>,
    pub download_bps: Option<u64>,
    pub upload_bps: Option<u64>,
    pub latency: Option<i64>,
    pub error: Option<String>,
}

/// 触发延迟测速（批量，使用 urltest 机制）
#[tauri::command]
pub async fn speedtest_run_latency(node_tags: Vec<String>) -> ApiResponse<()> {
    // TODO(模块E): 调用 speedtest::latency::run_latency_test()
    tracing::info!("延迟测速节点数: {}", node_tags.len());
    ApiResponse::ok(())
}

/// 单节点吞吐量测速
#[tauri::command]
pub async fn speedtest_run_single(node_tag: String) -> ApiResponse<ThroughputResult> {
    // TODO(模块E): 调用 speedtest::throughput::run_single()
    tracing::info!("单节点测速: {}", node_tag);
    ApiResponse::err("测速功能开发中（模块E）", 501)
}

/// 批量测速（串行）
#[tauri::command]
pub async fn speedtest_run_batch(group_tag: String) -> ApiResponse<()> {
    // TODO(模块E): 调用 speedtest::scheduler::run_batch()
    tracing::info!("批量测速分组: {}", group_tag);
    ApiResponse::err("批量测速功能开发中（模块E）", 501)
}

/// 取消批量测速
#[tauri::command]
pub async fn speedtest_cancel_batch() -> ApiResponse<()> {
    // TODO(模块E): 发送取消信号给 scheduler
    ApiResponse::ok(())
}

/// 获取测速结果列表
#[tauri::command]
pub async fn speedtest_get_results() -> ApiResponse<Vec<SpeedTestTask>> {
    // TODO(模块E): 从内存缓存或本地存储读取
    ApiResponse::ok(vec![])
}
