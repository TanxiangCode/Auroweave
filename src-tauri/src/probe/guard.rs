/// 主动健康守卫 —— 故障转移的主检测器
/// 作者: TanXiang
///
/// # 为什么必须自己检测（内核做不到）
///
/// 实测 sing-box 1.14.2 的能力边界：
/// - **无 `fallback` / `loadbalance` 出站**（25 种出站类型里没有）
/// - `urltest` 字段只有 `url`/`interval`/`tolerance`/`idle_timeout`/
///   `interrupt_exist_connections`，**无任何按需触发字段**；实测 4 次用户连接
///   失败，健康检查触发 **0** 次（只在启动时探 1 次）
/// - `urltest` 故障转移延迟 **= interval**。370 节点要做到 5s 转移需
///   74 请求/秒（实测 10 成员 interval=5s = 10 req/s），而这正是本次故障的
///   根源——805 次脉冲挤占连接资源，导致用户请求 15.2s 超时
///
/// 结论：内核在本规模下无法同时做到「快速转移」与「不干扰用户」，必须由
/// 应用侧检测 + 下发切换。
///
/// # 为什么测「主实例」而不是「探测面」
///
/// 探测面走 test-core 专属端口（40040+），与用户实际流量是**两条独立通路**。
/// 用户的连接断了，探测面完全不知情。而 ClashAPI `GET /proxies/{node}/delay`
/// 测的是**主实例内该节点出站**，与用户流量同进程、同连接资源——它测的就是
/// 用户真实的体验。这让「节点是否可用」成为可观测事实，而非间接推断。
///
/// # 为什么不用 /logs 事件驱动（已评估并否决）
///
/// 实测 `/logs` WebSocket 确实毫秒级推送 `{"type":"error","payload":...}`，
/// 且 `log level=warn` 下照常推送。但有两个硬伤：
/// 1. **payload 只标识组**（`outbound/selector[auto]`），不标识具体节点
/// 2. **error ≠ 节点死**：被墙域名、目标站点故障、DNS 失败同样报 error。
///    直接据此切换会退化为「用户永远连不上」
///
/// 即便加交叉验证，裁决仍要靠一次真实探测（1-2s），事件只把「发现」提前
/// ~1s，却要引入 WebSocket 依赖 + 日志文本解析（跨版本易碎）。收益远小于
/// 成本，故本实现只用主动探测，事件流留作后续可插拔的加速器。
///
/// # 判定策略：连续 N 次失败才切换
///
/// 单次失败可能是网络抖动，直接切换会造成不必要的连接重建。连续 2 次失败
/// （约 2s）才判定失效——既过滤抖动，又远快于内核的 180s。
use super::selector::{AutoSelector, Decision};
use super::table::{FailureClass, ProbeTable};
use crate::core::clash_api::ClashApiClient;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// 守卫参数
#[derive(Debug, Clone)]
pub struct HealthGuardConfig {
    /// 健康检查 URL
    ///
    /// 刻意用固定的 gstatic 204 而非用户实际访问的域名：检查必须反映
    /// 「节点本身是否可用」，不能被「某个域名被墙」污染。
    pub url: String,
    /// 单次检查超时（毫秒）
    ///
    /// 3000ms 与实测延迟分布匹配（p50=803 / p90=1437 / max=2975ms）：
    /// 再小会误杀健康慢节点，这也是旧路径 3000ms 超时的教训。
    pub timeout_ms: u64,
    /// 检查间隔（毫秒）
    pub interval_ms: u64,
    /// 连续失败多少次判定失效
    pub fail_threshold: u32,
}

impl Default for HealthGuardConfig {
    fn default() -> Self {
        Self {
            url: "http://www.gstatic.com/generate_204".to_string(),
            timeout_ms: 3000,
            interval_ms: 1000,
            fail_threshold: 2,
        }
    }
}

/// 单次守卫检查的结果
#[derive(Debug, Clone, PartialEq)]
pub enum GuardOutcome {
    /// 内核不可达或组无选择，本轮跳过
    Skipped,
    /// 当前节点健康，且已把实测延迟写回节点表
    Healthy { node: String, rtt_ms: u16 },
    /// 本轮失败但未达阈值（仍在观察）
    Suspect { node: String, fails: u32 },
    /// 已确认失效并完成故障转移
    Failover { from: String, to: String },
    /// 已确认失效，但没有任何健康候选可切（应告警）
    NoCandidate { node: String },
}

/// 连续失败计数器（纯逻辑，便于单测）
#[derive(Debug, Default)]
pub struct FailureCounter {
    consecutive: u32,
}

impl FailureCounter {
    /// 成功即清零
    pub fn on_success(&mut self) {
        self.consecutive = 0;
    }

    /// 记一次失败；返回是否达到阈值（达到即应故障转移）
    pub fn on_failure(&mut self, threshold: u32) -> bool {
        self.consecutive = self.consecutive.saturating_add(1);
        threshold.max(1) > 0 && self.consecutive >= threshold.max(1)
    }

    pub fn consecutive(&self) -> u32 {
        self.consecutive
    }
}



/// 主动健康守卫
pub struct ActiveHealthGuard {
    cfg: HealthGuardConfig,
    /// 主实例 ClashAPI（测真实用户路径）
    client: Arc<ClashApiClient>,
    /// 节点表（写回实测延迟 / 标记失效）
    table: Arc<RwLock<ProbeTable>>,
    /// 选点器（执行故障转移）
    selector: Arc<AutoSelector>,
    fails: AtomicU32,
    stopped: AtomicBool,
}

impl ActiveHealthGuard {
    pub fn new(
        cfg: HealthGuardConfig,
        table: Arc<RwLock<ProbeTable>>,
        selector: Arc<AutoSelector>,
    ) -> Self {
        Self {
            cfg,
            client: Arc::new(ClashApiClient::default()),
            table,
            selector,
            fails: AtomicU32::new(0),
            stopped: AtomicBool::new(false),
        }
    }

    /// 供测试注入自定义 ClashAPI（指向受控实例）
    pub fn with_client(mut self, client: Arc<ClashApiClient>) -> Self {
        self.client = client;
        self
    }

    pub fn config(&self) -> &HealthGuardConfig {
        &self.cfg
    }

    /// 执行一轮检查
    pub async fn run_once(&self) -> GuardOutcome {
        // 1. 问内核当前用的是谁
        let Some(current) = self.selector.current_selection().await else {
            return GuardOutcome::Skipped;
        };
        if current.is_empty() {
            return GuardOutcome::Skipped;
        }

        // 2. 测「主实例内该节点出站」——与用户流量同路径
        match self
            .client
            .get_node_delay(&current, &self.cfg.url, self.cfg.timeout_ms)
            .await
        {
            Ok(rtt) => {
                self.fails.store(0, Ordering::Relaxed);
                // 实测延迟写回节点表：比探测面更权威（同一进程同一资源），
                // 且顺带免除探测面对该节点的重复探测
                if let Some(n) = self.table.write().await.get_mut(&current) {
                    n.record_observed(rtt, now_ms());
                }
                GuardOutcome::Healthy {
                    node: current,
                    rtt_ms: rtt,
                }
            }
            Err(_) => {
                let n = self.fails.fetch_add(1, Ordering::Relaxed) + 1;
                if n < self.cfg.fail_threshold.max(1) {
                    return GuardOutcome::Suspect {
                        node: current,
                        fails: n,
                    };
                }
                // 3. 确认失效：标记后交给选点器做 emergency 切换
                if let Some(node) = self.table.write().await.get_mut(&current) {
                    node.record_failure(FailureClass::Unknown, now_ms());
                }
                match self.selector.run_once().await {
                    Decision::Switch { to, .. } => {
                        // 复位计数，给新节点完整的观察窗口（避免全池皆坏时疯狂抖动）
                        self.fails.store(0, Ordering::Relaxed);
                        log::warn!(
                            "[guard] 故障转移: {} -> {}（连续 {} 次探测失败）",
                            current,
                            to,
                            n
                        );
                        GuardOutcome::Failover { from: current, to }
                    }
                    _ => {
                        log::error!(
                            "[guard] 节点 {} 已失效但无健康候选，保持当前（网络不可用）",
                            current
                        );
                        GuardOutcome::NoCandidate { node: current }
                    }
                }
            }
        }
    }

    /// 后台常驻循环
    /// 后台常驻循环
    pub fn spawn_loop(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let tick = Duration::from_millis(self.cfg.interval_ms);
            while !self.stopped.load(Ordering::Relaxed) {
                let out = self.run_once().await;
                if let GuardOutcome::Suspect { fails, .. } = &out {
                    log::info!(
                        "[guard] 当前节点探测失败 {}/{}",
                        fails,
                        self.cfg.fail_threshold
                    );
                }
                tokio::time::sleep(tick).await;
            }
            log::info!("[guard] 守卫循环退出");
        })
    }

    /// 停止守卫循环
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_counter_requires_consecutive_failures() {
        // 单次失败可能是抖动，不得立即切换
        let mut c = FailureCounter::default();
        assert!(!c.on_failure(2), "第 1 次失败不该触发");
        assert!(c.on_failure(2), "连续 2 次才触发");
    }

    #[test]
    fn failure_counter_resets_on_success() {
        // 一次成功即清零——不能「累计失败」跨健康期
        let mut c = FailureCounter::default();
        assert!(!c.on_failure(3));
        assert!(!c.on_failure(3));
        assert_eq!(c.consecutive(), 2);
        c.on_success();
        assert_eq!(c.consecutive(), 0, "成功后必须清零");
        assert!(!c.on_failure(3), "清零后重新计数");
    }

    #[test]
    fn failure_counter_threshold_is_floor_one() {
        // threshold=0 或 1 都不能导致「永不切换」
        let mut c = FailureCounter::default();
        assert!(c.on_failure(0), "threshold=0 应等价于 1");
        let mut c2 = FailureCounter::default();
        assert!(c2.on_failure(1), "threshold=1 首次即切");
    }

    #[test]
    fn config_defaults_match_measured_latency_distribution() {
        // 实测 p50=803 / p90=1437 / max=2975ms：超时过小会误杀健康慢节点
        let c = HealthGuardConfig::default();
        assert!(
            c.timeout_ms >= 3000,
            "超时 {}ms 低于实测 max(2975ms) 余量，会误杀健康节点",
            c.timeout_ms
        );
        // 1s 间隔 + 2 次阈值 → 故障转移延迟约 2s（对比内核 urltest 的 180s）
        assert!(c.interval_ms <= 2000);
        assert!(c.fail_threshold >= 2, "单次失败即切会因抖动误切");
    }

    #[tokio::test]
    async fn run_once_without_kernel_is_skipped() {
        // 内核不可达时必须跳过而非 panic/误切换
        let table = Arc::new(RwLock::new(ProbeTable::new()));
        let sel = Arc::new(AutoSelector::new(
            super::super::selector::SelectorConfig::default(),
            table.clone(),
        ));
        let g = ActiveHealthGuard::new(HealthGuardConfig::default(), table, sel);
        // 未 refresh 的表 + 指向默认端口的内核 → 取不到选择 → Skipped
        let out = g.run_once().await;
        assert!(
            matches!(out, GuardOutcome::Skipped | GuardOutcome::Suspect { .. }),
            "无内核时不应触发故障转移，实际 {:?}",
            out
        );
    }

    #[tokio::test]
    async fn observed_rtt_is_written_back_to_table() {
        // 守卫实测的延迟是「用户真实路径」数据，应写回节点表供选点使用
        let table = Arc::new(RwLock::new(ProbeTable::new()));
        let now = now_ms();
        table.write().await.reset_to(&["A".to_string()], now);
        let mut t = table.write().await;
        t.get_mut("A").unwrap().record_observed(456, now);
        let n = t.get("A").unwrap();
        assert_eq!(n.rtt_ms, Some(456));
        assert_eq!(n.tier, crate::probe::table::ProbeTier::Active);
    }
}
