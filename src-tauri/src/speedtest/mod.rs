/// Auroweave 智能测速模块
/// 作者: TanXiang
pub mod throughput;
pub mod scheduler;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputResult {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub tested_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgress {
    pub current_index: usize,
    pub total: usize,
    pub current_node: String,
    pub result: Option<ThroughputResult>,
}
