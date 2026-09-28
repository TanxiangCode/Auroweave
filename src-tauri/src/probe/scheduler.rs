/// 探测调度器 —— 在常驻 test-core 上跑分层/抖动/退避的探测循环
/// 作者: TanXiang
///
/// 关键约束：**探测流量绝不经过主实例**。旧架构下 805 次探测挤在 180s 边界的
/// 瞬间、经主实例出站发出，实测导致用户真实请求 15.2s 超时（338 次）。本调度器
/// 把探测放到独立进程的专属端口上，实测 64 并发探测期间主实例 mixed 端口
/// 响应 112ms。
///
/// 生命周期：应用启动时 refresh 一次，订阅刷新时再次 refresh（指纹未变则零
/// 重启），退出时随进程树回收。
use super::table::{FailureClass, ProbeTable};
use crate::core::test_core::{TestCoreManager, PROBE_PLANE_MAX_NODES, PROBE_PLANE_PORT_BASE};
use crate::core::parser::ParsedOutbound;
use log::info;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// 探测结果分类
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProbeOutcome {
    /// 成功，携带延迟毫秒
    Success(u16),
    /// 失败，携带分类
    Failure(FailureClass),
}

/// 调度参数
#[derive(Debug, Clone)]
pub struct ProbeConfig {
    /// 单节点探测超时（毫秒）
    ///
    /// 默认 5000 而非旧路径的 3000：实测该订阅延迟 p50=803ms / p90=1437ms /
    /// max=2975ms，3000ms 预算下大量健康节点贴着超时线被误杀，随后又要花
    /// 「超时+退避+重试」的代价重新确认一遍。
    pub timeout_ms: u64,
    /// 全局并发上限
    pub concurrency: usize,
    /// 单轮最多探测多少节点（削峰：避免一次 tick 突发）
    pub max_per_round: usize,
    /// 调度 tick 间隔
    pub tick_ms: u64,
    /// 探测 URL
    pub url: String,
    /// 结果有效期（毫秒）：超过则不参与"最优"评选
    pub fresh_ms: i64,
    /// Standby 层间隔（毫秒；用户可配）
    pub standby_interval_ms: i64,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            concurrency: 64,
            max_per_round: 64,
            tick_ms: 1000,
            url: "http://www.gstatic.com/generate_204".to_string(),
            fresh_ms: 120_000,
            standby_interval_ms: 300_000,
        }
    }
}

impl ProbeConfig {
    /// 覆盖 Standby 层间隔（用户「分组测速设置」里的 interval 入口）
    ///
    /// 下界 30s：低于此值 370 节点的探测率会高到像回到了旧架构的脉冲模式。
    pub fn with_standby_interval_ms(mut self, ms: i64) -> Self {
        self.standby_interval_ms = ms.clamp(30_000, 3_600_000);
        self
    }

    /// 派生分层间隔
    pub fn tier_intervals(&self) -> super::table::TierIntervals {
        super::table::TierIntervals {
            // Active 层固定 60s，不接受用户覆盖：正在用的出口必须保持新鲜
            active_ms: 60_000,
            standby_ms: self.standby_interval_ms,
            cooling_ms: 30_000,
        }
    }
}

/// 探测调度器
pub struct ProbeScheduler {
    config: ProbeConfig,
    /// 节点表（唯一真相源）
    table: Arc<RwLock<ProbeTable>>,
    /// 常驻探测内核
    core: Arc<TestCoreManager>,
    /// tag → 专属端口（与 test-core 配置生成顺序一致）
    port_of: Arc<RwLock<HashMap<String, u16>>>,
    /// 选点器：`auto` 改 selector 后的决策方（可为空——内核不可用时跳过）
    selector: Arc<tokio::sync::Mutex<Option<Arc<super::selector::AutoSelector>>>>,
    stopped: Arc<AtomicBool>,
}

impl ProbeScheduler {
    pub fn new(config: ProbeConfig) -> Self {
        Self {
            config,
            table: Arc::new(RwLock::new(ProbeTable::new())),
            core: Arc::new(TestCoreManager::new()),
            port_of: Arc::new(RwLock::new(HashMap::new())),
            selector: Arc::new(tokio::sync::Mutex::new(None)),
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 挂载选点器：`auto` 改 selector 后由它决定选谁
    pub async fn attach_selector(&self, sel: Arc<super::selector::AutoSelector>) {
        *self.selector.lock().await = Some(sel);
    }

    /// 执行一轮选点（内部调用；选点器未挂载时静默跳过）
    ///
    /// 选点失败绝不影响代理可用性：auto 此刻仍持有内核恢复的上次选择，
    /// 那是一个已知可用的节点——不动就是「维持现状」。
    pub async fn run_selection(&self) -> Option<super::selector::Decision> {
        let sel = self.selector.lock().await.clone()?;
        let d = sel.run_once().await;
        // 把内核当前使用的节点 pin 住：它必须以 Active 节奏（60s、不退避）
        // 被探测——这是故障转移信号的唯一来源（探测面看不到用户流量断开）。
        if let Some(cur) = sel.current_selection().await {
            self.table.write().await.pin_active(&cur, now_ms());
        }
        Some(d)
    }

    /// 只读访问节点表（供组视图查询「谁最快」）
    pub fn table(&self) -> Arc<RwLock<ProbeTable>> {
        self.table.clone()
    }

    pub fn config(&self) -> &ProbeConfig {
        &self.config
    }

    /// 用订阅节点集重设表并确保常驻内核就绪
    ///
    /// 节点集未变时 `ensure_running` 零重启（指纹命中直接返回既有基址）。
    /// `restore_history` 为 true 时从 `stats.dat` 回填历史延迟（仅首次启动，
    /// 订阅刷新场景保留内存中已有状态，避免用旧数据覆盖新结果）。
    pub async fn refresh(
        &self,
        nodes: Vec<ParsedOutbound>,
        restore_history: bool,
    ) -> Result<(), crate::error::AppError> {
        if nodes.len() > PROBE_PLANE_MAX_NODES {
            return Err(crate::error::AppError::Validation(format!(
                "节点数 {} 超出探测面端口段容量 {}，请检查订阅",
                nodes.len(),
                PROBE_PLANE_MAX_NODES
            )));
        }

        // tag → 端口（与 build_test_config 的枚举顺序严格一致）
        let mut ports = HashMap::with_capacity(nodes.len());
        for (i, n) in nodes.iter().enumerate() {
            ports.insert(n.tag.clone(), PROBE_PLANE_PORT_BASE + i as u16);
        }

        let tags: Vec<String> = nodes.iter().map(|n| n.tag.clone()).collect();
        {
            let mut t = self.table.write().await;
            t.set_intervals(self.config.tier_intervals());
            t.reset_to(&tags, now_ms());
            // 回填持久化历史：重启后不必把 370 个节点全部重探一遍。
            // 只在「表刚建立」时回填（订阅刷新时表已保留存量状态，
            // 再回填会用旧数据覆盖刚探到的新结果）。
            if restore_history {
                let records = crate::core::stats_db::load_probe_states();
                if !records.is_empty() {
                    let list: Vec<_> = records.into_values().collect();
                    t.restore_from(
                        &list,
                        now_ms(),
                        self.config.fresh_ms * 10, // 历史宽限：10×新鲜度
                    );
                    log::info!("[probe] 已回填 {} 条历史探测记录", list.len());
                }
            }
        }

        // 常驻内核（未变化则复用，零重启）
        self.core
            .ensure_running(&nodes, PROBE_PLANE_PORT_BASE)
            .await?;
        *self.port_of.write().await = ports;

        // 选点器成员同步：auto 组成员即全量有效节点。
        // 地区组 / 自定义组是它的子集，选点器按各自 tag 另建实例即可。
        if let Some(sel) = self.selector.lock().await.clone() {
            sel.set_members(tags.clone()).await;
        }

        let (healthy, total) = self
            .table
            .read()
            .await
            .health_summary(now_ms(), self.config.fresh_ms);
        info!(
            "[probe] 探测面就绪: {} 节点（健康 {}），端口段 {}",
            total,
            healthy,
            PROBE_PLANE_PORT_BASE
        );
        Ok(())
    }

    /// 把指定节点提为 Active（当前选中 / pinned）
    ///
    /// Active 层间隔 60s（Standby 300s）。**不参与退避**——见
    /// [`Self::pin_active`] 对「内核当前正在使用的节点」的特殊处理。
    pub async fn promote(&self, tags: &[String]) {
        self.table.write().await.promote(tags, now_ms());
    }

    /// 标记「内核当前正在使用的节点」并锁定其探测节奏
    ///
    /// 这是故障转移能成立的关键。探测面走 test-core 专属端口，与用户实际
    /// 流量是两条独立通路——**用户的连接断开，探测面无从得知**，只能靠下一轮
    /// 探测发现该节点已死。因此当前节点必须被优先、频繁地探测，且**不能退避**：
    ///
    /// - 退避会让故障转移延迟随失败次数指数增长（第 3 次失败要等 2 分钟才
    ///   重新确认，用户已经断了很久）
    /// - 故 `fail_streak` 归零、间隔钳在 `active_ms`（60s）
    ///
    /// 代价是「当前节点恰好被墙」时会持续消耗探测配额（60s 一次）——可接受，
    /// 因为它同时也是故障转移的信号来源。
    pub async fn pin_active(&self, tag: &str) {
        self.table.write().await.pin_active(tag, now_ms());
    }

    /// 执行一轮探测：取到期节点 → 受控并发探测 → 回写结果
    ///
    /// 返回本轮实际探测数量。`max_per_round` 保证单轮突发受控。
    pub async fn run_once(&self) -> usize {
        let now = now_ms();
        let due: Vec<String> = {
            let mut t = self.table.write().await;
            t.take_due(now, self.config.max_per_round)
        };
        if due.is_empty() {
            return 0;
        }

        let ports = self.port_of.read().await.clone();
        let mut tasks = Vec::with_capacity(due.len());
        for tag in due {
            let Some(port) = ports.get(&tag).copied() else {
                // 端口缺失（订阅竞态）：记不可达，交由退避消化
                let mut t = self.table.write().await;
                if let Some(n) = t.get_mut(&tag) {
                    n.record_failure(FailureClass::Unreachable, now_ms());
                }
                continue;
            };
            let url = self.config.url.clone();
            let timeout = Duration::from_millis(self.config.timeout_ms);
            tasks.push(tokio::spawn(async move {
                let out = probe_one(port, &url, timeout).await;
                (tag, out)
            }));
        }
        drop(ports);

        let mut probed = 0usize;
        let mut touched: Vec<String> = Vec::new();
        {
            let mut t = self.table.write().await;
            for j in tasks {
                if let Ok((tag, outcome)) = j.await {
                    let now = now_ms();
                    if let Some(n) = t.get_mut(&tag) {
                        match outcome {
                            ProbeOutcome::Success(rtt) => n.record_success(rtt, now),
                            ProbeOutcome::Failure(c) => n.record_failure(c, now),
                        }
                    }
                    touched.push(tag);
                    probed += 1;
                }
            }
        }
        // 落盘本轮变化：重启后不必把 370 节点全部重探（只写变化的少数行）
        if !touched.is_empty() {
            let snapshot: Vec<_> = {
                let t = self.table.read().await;
                t.snapshot()
                    .into_iter()
                    .filter(|r| touched.contains(&r.node_tag))
                    .collect()
            };
            tokio::task::spawn_blocking(move || {
                crate::core::stats_db::upsert_probe_states(snapshot);
            });
        }
        probed
    }

    /// 后台调度循环（启动即常驻）
    ///
    /// 每 tick 两件事：先测量（探测到期节点），再选点（据表决策）。
    /// 顺序有意义——先有新数据，选点才有依据。
    pub fn spawn_loop(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let tick = Duration::from_millis(self.config.tick_ms);
            while !self.stopped.load(Ordering::Relaxed) {
                let n = self.run_once().await;
                if n > 0 {
                    log::debug!("[probe] 本轮探测 {} 个节点", n);
                    // 有新数据才值得选点；空轮不重复问内核（省一次 /proxies 全量拉取）
                    self.run_selection().await;
                }
                tokio::time::sleep(tick).await;
            }
            info!("[probe] 调度循环退出");
        })
    }

    /// 停止调度并回收常驻内核
    pub async fn shutdown(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        self.core.stop().await;
    }
}

impl Default for ProbeScheduler {
    fn default() -> Self {
        Self::new(ProbeConfig::default())
    }
}

/// 单节点探测：经专属端口发一次 HTTP，拿到合法响应即视为可达
///
/// 只判「能否拿到响应」——不比对状态码：不同节点对探测 URL 可能返回 204 也
/// 可能返回 3xx，判状态码会把可用节点误判为失败。
async fn probe_one(port: u16, url: &str, timeout: Duration) -> ProbeOutcome {
    let proxy = match reqwest::Proxy::all(format!("http://127.0.0.1:{port}")) {
        Ok(p) => p,
        Err(_) => return ProbeOutcome::Failure(FailureClass::Unknown),
    };
    let client = match reqwest::Client::builder()
        .proxy(proxy)
        .timeout(timeout)
        .build()
    {
        Ok(c) => c,
        Err(_) => return ProbeOutcome::Failure(FailureClass::Unknown),
    };

    let started = std::time::Instant::now();
    match client.get(url).send().await {
        Ok(resp) => {
            // 读完 body：确认响应完整，同时让连接可复用
            let _ = resp.bytes().await;
            let ms = started.elapsed().as_millis().clamp(1, u16::MAX as u128) as u16;
            ProbeOutcome::Success(ms)
        }
        Err(e) => ProbeOutcome::Failure(classify_error(&e)),
    }
}

/// 把 reqwest 错误归类为 [`FailureClass`]
///
/// 分类的意义在于**决定是否立即重试**：旧实现对所有失败统一「退避 400ms 重试
/// 一次」，而超时的节点重试仍然超时，纯属把耗时翻倍。
fn classify_error(e: &reqwest::Error) -> FailureClass {
    if e.is_timeout() {
        return FailureClass::Timeout;
    }
    if e.is_connect() {
        return FailureClass::Unreachable;
    }
    if e.is_builder() {
        return FailureClass::Dns;
    }
    let src = e.to_string().to_lowercase();
    if src.contains("tls") || src.contains("handshake") || src.contains("certificate") {
        FailureClass::Tls
    } else if src.contains("dns") || src.contains("resolve") || src.contains("name") {
        FailureClass::Dns
    } else if src.contains("refused") || src.contains("reset") || src.contains("unreachable") {
        FailureClass::Unreachable
    } else {
        FailureClass::Unknown
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// 便捷：取当前最优节点（供组视图使用）
pub async fn best_of(
    table: &Arc<RwLock<ProbeTable>>,
    members: &[String],
    fresh_ms: i64,
) -> Option<(String, u16)> {
    let t = table.read().await;
    t.best_of(members, now_ms(), fresh_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_measured_fleet_p50() {
        // 默认超时必须宽于实测 p90(1437ms) 与 max(2975ms)，否则健康节点被误杀
        let c = ProbeConfig::default();
        assert!(c.timeout_ms >= 5000, "超时过短会误杀健康节点");
        assert!(
            c.fresh_ms > c.timeout_ms as i64,
            "结果有效期须大于单次超时"
        );
        // 64 并发下 370 节点约 6 波，与实测（64 并发 5.15s 收尾）吻合
        assert!((32..=128).contains(&c.concurrency));
    }

    #[tokio::test]
    async fn refresh_rejects_pool_larger_than_port_segment() {
        // 端口段容量是硬约束：越界会溢出到吞吐测速段
        let sch = ProbeScheduler::default();
        let nodes: Vec<ParsedOutbound> = (0..PROBE_PLANE_MAX_NODES + 1)
            .map(|i| ParsedOutbound {
                tag: format!("n{i}"),
                r#type: "shadowsocks".to_string(),
                server: Some("x.example.com".to_string()),
                server_port: Some(8388),
                raw_json: serde_json::json!({}),
            })
            .collect();
        assert!(sch.refresh(nodes, false).await.is_err());
    }

    #[tokio::test]
    async fn run_once_on_empty_table_is_noop() {
        // 表为空时不得启动任何探测（也不得触碰内核）
        let sch = ProbeScheduler::default();
        assert_eq!(sch.run_once().await, 0);
    }
}
