/// 自动选点器 —— `auto` 组由 selector 承载后，由本模块决定选谁
/// 作者: TanXiang
///
/// # 为什么需要它
///
/// `auto` 从 `urltest` 改为 `selector` 后，内核不再「自己探自己选」——它变成一个
/// 纯粹的被控执行器。谁来选？本模块：读节点表（唯一真相源）→ 算 argmin(rtt)
/// → 经 ClashAPI `PUT /proxies/auto` 下发切换。
///
/// # 为什么断网暗窗接近 0
///
/// sing-box 的 `store_selected` 已废弃，**随 `cache_file.enabled` 默认生效**
/// （docs/experimental/clash-api.zh.md:128-138），即 selector 的当前选择被持久化。
/// 实测（sing-box 1.14.2 / macOS-arm64）：切到 C → 杀进程 → 重启，`now` 仍为 C。
/// 所以内核重启后自己就恢复了上次选择，不等本模块跑完第一轮探测。
///
/// # 迟滞：防止优选抖动
///
/// 最优节点每次探测都可能换人（网络抖动）。若无条件跟随，用户会经历「无感知但
/// 持续重建连接」——比不优选更糟。两条防线：
///
/// 1. **切换阈值**：新候选必须比当前节点快 `min_gain_ms` 以上才切。实测该订阅
///    p50=803ms / 候选间差距常在几十毫秒量级，阈值过小会天天切。
/// 2. **最小驻留时间**：切过之后 `hold_ms` 内不再动，给连接建立留出稳定窗口。
use super::table::ProbeTable;
use crate::core::clash_api::ClashApiClient;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 选点参数
#[derive(Debug, Clone)]
pub struct SelectorConfig {
    /// 被控的组 tag
    pub group_tag: String,
    /// 新候选至少要比当前快这么多毫秒才值得切换
    pub min_gain_ms: u16,
    /// 两次切换之间的最小间隔（防止连续抖动）
    pub hold_ms: i64,
    /// 候选新鲜度上限（毫秒）
    pub fresh_ms: i64,
}

impl Default for SelectorConfig {
    fn default() -> Self {
        Self {
            group_tag: "auto".to_string(),
            // 实测候选间延迟差异常在几十毫秒；50ms 阈值过滤噪声，
            // 又不至于让明显更快的节点（差 200ms+）被忽略
            min_gain_ms: 50,
            // 5 分钟驻留：与旧 urltest 的 3 分钟 interval 同量级，
            // 但因为只在「明显更优」时才切，实际切换频率远低于此
            hold_ms: 300_000,
            fresh_ms: 120_000,
        }
    }
}

/// 选点决策结果
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// 无需切换（已是当前最优 / 无新鲜候选 / 仍在驻留期）
    Hold,
    /// 切换到新节点
    ///
    /// `emergency` 为真表示这是**故障转移**（当前节点已确认失效），而非
    /// 常规优选——故障转移不受驻留期与增益阈值约束。
    Switch {
        from: String,
        to: String,
        rtt_ms: u16,
        emergency: bool,
    },
}

/// 纯函数式决策：给定当前节点（含其延迟与失效状态）与最优候选，产出决策
///
/// 抽成纯函数是为了能单测——选点逻辑是「用户网络体验」的最后一道闸，
/// 任何误判都直接表现为用户被切到坏节点，或卡死在坏节点上。
///
/// # 两类切换，两套规则
///
/// | | 常规优选 | 故障转移（`current_failed`） |
/// |---|---|---|
/// | 触发 | 别的节点明显更快 | 当前节点已被探测确认失效 |
/// | 驻留期 | 遵守 | **忽略**（不可用时谈驻留没意义） |
/// | 增益阈值 | 遵守 | **忽略**（没有更快的也要切） |
/// | 语义 | 优化 | 可用性 |
///
/// `current_rtt` 为 `None` 且 `current_failed` 为 false 的场景是「用户手选了一个
/// 尚未探测过的节点」——无从判断其存活，走常规优选（否则会误判为失效而频繁切换）。
pub fn decide(
    current: &str,
    current_rtt: Option<u16>,
    current_failed: bool,
    best: Option<(&str, u16)>,
    cfg: &SelectorConfig,
    last_switch_at: Option<i64>,
    now: i64,
) -> Decision {
    let Some((candidate, rtt)) = best else {
        // 无新鲜候选：宁可保持当前，也不要切到陈旧数据指向的节点
        return Decision::Hold;
    };
    if candidate == current {
        return Decision::Hold;
    }

    // 故障转移：当前节点已确认失效 → 无视迟滞，立刻切
    if current_failed {
        return Decision::Switch {
            from: current.to_string(),
            to: candidate.to_string(),
            rtt_ms: rtt,
            emergency: true,
        };
    }

    // 常规优选的迟滞：驻留期
    if let Some(t) = last_switch_at {
        if now - t < cfg.hold_ms {
            return Decision::Hold;
        }
    }
    // 常规优选的迟滞：增益不足（用 u32 避免 u16 加法溢出）
    if let Some(cur) = current_rtt {
        if (rtt as u32) + (cfg.min_gain_ms as u32) > cur as u32 {
            return Decision::Hold;
        }
    }
    Decision::Switch {
        from: current.to_string(),
        to: candidate.to_string(),
        rtt_ms: rtt,
        emergency: false,
    }
}

/// 自动选点器
pub struct AutoSelector {
    cfg: SelectorConfig,
    table: Arc<RwLock<ProbeTable>>,
    client: Arc<ClashApiClient>,
    members: RwLock<Vec<String>>,
    last_switch_at: RwLock<Option<i64>>,
}

impl AutoSelector {
    pub fn new(cfg: SelectorConfig, table: Arc<RwLock<ProbeTable>>) -> Self {
        Self {
            cfg,
            table,
            client: Arc::new(ClashApiClient::default()),
            members: RwLock::new(Vec::new()),
            last_switch_at: RwLock::new(None),
        }
    }

    /// 注入自定义 ClashAPI（测试指向受控内核实例时使用）
    pub fn with_client(mut self, client: Arc<ClashApiClient>) -> Self {
        self.client = client;
        self
    }

    /// 设置候选成员（组内节点列表）
    pub async fn set_members(&self, members: Vec<String>) {
        *self.members.write().await = members;
    }

    pub fn config(&self) -> &SelectorConfig {
        &self.cfg
    }

    /// 读内核当前选择
    pub async fn current_selection(&self) -> Option<String> {
        let json = self.client.get_proxies().await.ok()?;
        json.get("proxies")?
            .get(&self.cfg.group_tag)?
            .get("now")?
            .as_str()
            .map(|s| s.to_string())
    }

    /// 执行一轮选点：读表 → 决策 → 下发
    pub async fn run_once(&self) -> Decision {
        let now = now_ms();
        let members = self.members.read().await.clone();
        if members.is_empty() {
            return Decision::Hold;
        }

        // 当前选择以内核为准（用户可能手动切过）；内核不可用则本轮不动
        let current = match self.current_selection().await {
            Some(c) if !c.is_empty() => c,
            _ => return Decision::Hold,
        };

        // 当前节点的新鲜延迟 + 是否已被探测确认失效
        let (current_rtt, current_failed, best) = {
            let t = self.table.read().await;
            let node = t.get(&current);
            let cur = node.and_then(|n| n.effective_rtt(now, self.cfg.fresh_ms));
            // 「最近一次探测失败」才是失效判据（last_probe_failed）。
            // 不能用 rtt.is_none()：从未探测过的新节点同样为 None。
            let failed = node.map(|n| n.last_probe_failed()).unwrap_or(false);
            let b = t.best_of(&members, now, self.cfg.fresh_ms);
            (cur, failed, b)
        };
        let best_ref = best.as_ref().map(|(tag, rtt)| (tag.as_str(), *rtt));

        let d = decide(
            &current,
            current_rtt,
            current_failed,
            best_ref,
            &self.cfg,
            *self.last_switch_at.read().await,
            now,
        );

        if let Decision::Switch { to, rtt_ms, emergency, .. } = &d {
            match self.client.select_node(&self.cfg.group_tag, to).await {
                Ok(()) => {
                    *self.last_switch_at.write().await = Some(now);
                    if *emergency {
                        log::warn!(
                            "[selector] 故障转移：{} 已失效，{} -> {}（{}ms）",
                            self.cfg.group_tag,
                            current,
                            to,
                            rtt_ms
                        );
                    } else {
                        log::info!(
                            "[selector] {} 切换: {} -> {}（{}ms）",
                            self.cfg.group_tag,
                            current,
                            to,
                            rtt_ms
                        );
                    }
                }
                Err(e) => {
                    // 切换失败不回滚：内核仍用旧选择，那仍是一个已知节点。
                    // 不重试是刻意的——下一轮 tick 会再决策，避免在此处
                    // 阻塞调度循环（若内核正在重启，重试也只会连续失败）。
                    log::warn!("[selector] 切换到 {} 失败: {}", to, e);
                    return Decision::Hold;
                }
            }
        }
        d
    }

    /// 选出最优但不下发（供前端「推荐节点」展示）
    pub async fn recommend(&self) -> Option<(String, u16)> {
        let members = self.members.read().await.clone();
        let t = self.table.read().await;
        t.best_of(&members, now_ms(), self.cfg.fresh_ms)
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::table::{FailureClass, NodeProbe, ProbeTable};

    const NOW: i64 = 1_700_000_000_000;

    fn cfg() -> SelectorConfig {
        SelectorConfig::default()
    }

    #[test]
    fn no_candidate_means_hold_never_switch_to_stale() {
        // 陈旧的"最优"比没有最优更危险：会把用户切到已下线节点
        assert_eq!(decide("A", Some(100), false, None, &cfg(), None, NOW), Decision::Hold);
    }

    #[test]
    fn same_node_is_hold() {
        assert_eq!(decide("A", Some(100), false, Some(("A", 100)), &cfg(), None, NOW), Decision::Hold);
    }

    #[test]
    fn switches_when_candidate_is_clearly_better() {
        let d = decide("A", Some(900), false, Some(("B", 200)), &cfg(), None, NOW);
        assert_eq!(
            d,
            Decision::Switch { from: "A".into(), to: "B".into(), rtt_ms: 200, emergency: false }
        );
    }

    #[test]
    fn marginal_gain_does_not_switch() {
        // 只快 30ms < 阈值 50ms：属于噪声，不值得重建用户连接
        let d = decide("A", Some(800), false, Some(("B", 770)), &cfg(), None, NOW);
        assert_eq!(d, Decision::Hold);
    }

    #[test]
    fn gain_exactly_at_threshold_switches() {
        // 800-750=50 == min_gain_ms：边界上应切
        let d = decide("A", Some(800), false, Some(("B", 750)), &cfg(), None, NOW);
        assert!(matches!(d, Decision::Switch { .. }));
    }

    #[test]
    fn unknown_current_rtt_switches_without_gain_check() {
        // 用户手选了未探测节点 → 无从比较起，有候选就切
        let d = decide("A", None, false, Some(("B", 800)), &cfg(), None, NOW);
        assert!(matches!(d, Decision::Switch { .. }));
    }

    #[test]
    fn respects_hold_window() {
        // 刚切过 1 分钟内不动
        let d = decide("B", Some(800), false, Some(("C", 100)), &cfg(), Some(NOW - 60_000), NOW);
        assert_eq!(d, Decision::Hold, "驻留期内应保持不动");
    }

    #[test]
    fn allows_switch_after_hold_window() {
        let d = decide("B", Some(800), false, Some(("C", 100)), &cfg(), Some(NOW - 400_000), NOW);
        assert!(matches!(d, Decision::Switch { .. }));
    }

    #[test]
    fn extreme_rtt_does_not_overflow() {
        // u16 加法溢出防护：65535 + 50 在 u16 下会回绕成 85，从而误判为「不够快」
        let d = decide("A", Some(u16::MAX), false, Some(("B", 1)), &cfg(), None, NOW);
        assert!(matches!(d, Decision::Switch { .. }));
    }

    // ---- 故障转移（2026-09-28 回归）----
    //
    // 背景：auto 从 urltest 改为 selector 后，「节点失效自动切换」的责任
    // 落到本模块。首次实现存在致命缺陷——record_failure 不清 rtt_ms，导致
    // 死节点仍以旧延迟参与评选并被持续选中，故障转移**完全不触发**。
    // 下列测试即该缺陷的回归防线。

    #[test]
    fn failure_invalidates_stored_rtt_immediately() {
        // 死节点绝不能继续携带旧延迟参与评选
        let mut n = NodeProbe::new("A", NOW);
        n.record_success(500, NOW);
        assert_eq!(n.rtt_ms, Some(500));
        n.record_failure(FailureClass::Unreachable, NOW + 1000);
        assert_eq!(n.rtt_ms, None, "失败后必须作废 rtt_ms");
        assert_eq!(n.last_ok_at, None, "失败后必须作废 last_ok_at");
        assert!(
            n.effective_rtt(NOW + 1000, 120_000).is_none(),
            "失效节点的 effective_rtt 必须为 None，否则会被当成健康候选"
        );
    }

    #[test]
    fn dead_node_is_never_selected_as_best() {
        let mut t = ProbeTable::new();
        let members: Vec<String> = vec!["A".into(), "B".into(), "C".into()];
        t.reset_to(&members, NOW);
        t.get_mut("A").unwrap().record_success(500, NOW);
        t.get_mut("B").unwrap().record_success(700, NOW);
        t.get_mut("C").unwrap().record_success(900, NOW);
        // A 彻底断线
        t.get_mut("A").unwrap().record_failure(FailureClass::Unreachable, NOW + 1000);

        let best = t.best_of(&members, NOW + 1000, 120_000);
        assert_eq!(
            best.map(|(tag, _)| tag),
            Some("B".to_string()),
            "死节点 A 不得被选为最优"
        );
    }

    #[test]
    fn emergency_switch_ignores_hold_window() {
        // 当前节点已失效 → 即使刚切过（驻留期内）也必须立刻切
        let d = decide(
            "A",
            None,
            true, // current_failed
            Some(("C", 2000)), // 候选反而更慢，但总比没有强
            &cfg(),
            Some(NOW - 10_000), // 刚切过 10 秒
            NOW,
        );
        match d {
            Decision::Switch { to, emergency, .. } => {
                assert_eq!(to, "C");
                assert!(emergency, "失效切换必须标记为 emergency");
            }
            Decision::Hold => panic!("当前节点已失效却判 Hold，故障转移失效"),
        }
    }

    #[test]
    fn emergency_switch_ignores_gain_threshold() {
        // 候选只快 10ms（远低于 50ms 阈值）→ 常规优选会 Hold，
        // 但当前节点已死，必须切
        let d = decide("A", None, true, Some(("B", 990)), &cfg(), None, NOW);
        assert!(
            matches!(d, Decision::Switch { emergency: true, .. }),
            "失效时不得被增益阈值阻挡"
        );
    }

    #[test]
    fn non_failed_unknown_node_still_respects_hysteresis() {
        // 反向保护：current_rtt=None 但 current_failed=false
        // （用户手选了从未探测过的节点）→ 走常规优选，不得当成失效
        let d = decide("A", None, false, Some(("B", 100)), &cfg(), Some(NOW - 1_000), NOW);
        assert_eq!(d, Decision::Hold, "驻留期内不得因未探节点而切换");
    }

    #[test]
    fn last_probe_failed_distinguishes_never_probed_from_failed() {
        let mut n = NodeProbe::new("A", NOW);
        // 从未探测
        assert!(!n.last_probe_failed(), "从未探测 ≠ 失效");
        // 探测失败
        n.record_failure(FailureClass::Timeout, NOW + 1000);
        assert!(n.last_probe_failed(), "探过且失败 = 失效");
        // 之后又成功了
        n.record_success(300, NOW + 2000);
        assert!(!n.last_probe_failed(), "恢复成功后不再是失效状态");
    }

    #[test]
    fn active_node_failure_does_not_back_off() {
        // 当前节点失败后必须在 active_ms 内重探。
        // 若退避，故障转移延迟会随失败次数指数增长（第 3 次失败等 2 分钟），
        // 用户已经断了很久——这是「节点失效能否自动切换」的核心约束。
        let mut t = ProbeTable::new();
        t.reset_to(&["A".to_string()], NOW);
        t.pin_active("A", NOW);
        let mut cur = NOW;
        for i in 0..5 {
            t.get_mut("A").unwrap().record_failure(FailureClass::Timeout, cur);
            let gap = t.get("A").unwrap().next_due_at - cur;
            assert!(
                gap <= 60_000,
                "第 {} 次失败后退避 {}ms 超过 active 间隔，故障转移会被拖慢",
                i + 1,
                gap
            );
            cur += gap;
        }
    }

    #[test]
    fn standby_node_failure_still_backs_off() {
        // 反向保护：非当前节点仍应退避，否则 370 个坏节点会持续消耗探测配额
        let mut t = ProbeTable::new();
        t.reset_to(&["B".to_string()], NOW);
        let mut gaps = Vec::new();
        let mut cur = NOW;
        for _ in 0..3 {
            t.get_mut("B").unwrap().record_failure(FailureClass::Timeout, cur);
            let gap = t.get("B").unwrap().next_due_at - cur;
            gaps.push(gap);
            cur += gap;
        }
        assert!(
            gaps.windows(2).all(|w| w[1] >= w[0]),
            "备用节点退避必须单调不减: {:?}",
            gaps
        );
        assert!(gaps[2] > gaps[0], "备用节点必须真实退避: {:?}", gaps);
    }

    #[test]
    fn pin_active_makes_node_due_immediately() {
        // 当前节点必须优先被探测（它失效的信号最重要）
        let mut t = ProbeTable::new();
        t.reset_to(&["A".to_string()], NOW);
        // 人为推后
        t.get_mut("A").unwrap().next_due_at = NOW + 300_000;
        t.pin_active("A", NOW);
        assert_eq!(t.get("A").unwrap().next_due_at, NOW, "pin 后应立即到期");
        assert!(t.get("A").unwrap().is_due(NOW));
    }

    #[tokio::test]
    async fn recommend_reads_single_source_of_truth() {
        let mut t = ProbeTable::new();
        let members: Vec<String> = vec!["a".into(), "b".into()];
        let now = now_ms();
        t.reset_to(&members, now);
        t.get_mut("a").unwrap().record_success(900, now);
        t.get_mut("b").unwrap().record_success(300, now);
        let table = Arc::new(RwLock::new(t));
        let sel = AutoSelector::new(cfg(), table);
        sel.set_members(members).await;
        let rec = sel.recommend().await;
        assert_eq!(rec.map(|(tag, _)| tag), Some("b".to_string()));
    }

}
