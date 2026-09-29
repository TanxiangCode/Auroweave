/// 节点延迟表 —— 探测结果的唯一真相源
/// 作者: TanXiang
///
/// 内存态 + 落盘（stats.dat）。所有「组」都只是这张表的视图：组不再各自发起
/// 探测，只按成员过滤本表来回答「谁最快」。
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// 探测分层：决定该节点的下次探测间隔
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeTier {
    /// 当前选中 / pinned / top-N：用户随时可能在用，必须新鲜
    Active,
    /// 健康备用：偶尔刷新即可
    Standby,
    /// 连续失败：指数退避，长时间失败后近乎停止
    Cooling,
}

impl ProbeTier {
    /// 本层基准间隔（毫秒）
    pub fn base_interval_ms(self) -> i64 {
        match self {
            ProbeTier::Active => 60_000,
            ProbeTier::Standby => 300_000,
            // Cooling 的实际间隔由 fail_streak 指数放大，基准取退避首档
            ProbeTier::Cooling => 30_000,
        }
    }
}

/// 分层间隔覆盖（用户可配）
///
/// 用户在「分组测速设置」里设的 interval 现在作用于探测面，而不是内核 urltest
/// （配置已不再生成 urltest 出站）。Active 层**不**接受覆盖——正在使用的节点
/// 必须保持新鲜，否则用户设 30 分钟会让当前出口半小时不校验。
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct TierIntervals {
    pub active_ms: i64,
    pub standby_ms: i64,
    pub cooling_ms: i64,
}

impl Default for TierIntervals {
    fn default() -> Self {
        Self {
            active_ms: 60_000,
            standby_ms: 300_000,
            cooling_ms: 30_000,
        }
    }
}

/// 失败分类 —— 决定退避行为
///
/// 旧实现对所有失败一视同仁（`delay==0 → sleep 400ms → 重试一次`）。但实测
/// 延迟分布 p50=803ms / p90=1437ms / max=2975ms，超时预算 3000ms——**超时的
/// 节点重试仍然超时**，重试只是把耗时翻倍。真正值得立即重试的只有瞬态类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// 探测超时：节点慢或已死。退避，不立即重试（旧实现的重试在这里纯属浪费）
    Timeout,
    /// 连接被拒 / 不可达：节点已下线。快速退避
    Unreachable,
    /// DNS 解析失败：节点配置问题，重试无用。快速退避
    Dns,
    /// TLS 握手失败：唯一值得立即重试的瞬态类
    Tls,
    /// 其他/未知
    Unknown,
}

impl FailureClass {
    /// 是否值得立即重试一次（旧实现对全部失败重试，这是唯一的例外）
    pub fn worth_retry_now(self) -> bool {
        matches!(self, FailureClass::Tls)
    }

    /// 退避加速系数：越"确定是节点坏了"退避越快
    pub fn backoff_weight(self) -> u32 {
        match self {
            FailureClass::Tls => 1,      // 可能是抖动，温和
            FailureClass::Timeout => 2,  // 大概率真慢
            FailureClass::Unknown => 2,
            FailureClass::Unreachable => 3, // 基本可判定下线
            FailureClass::Dns => 4,        // 配置错，重试无意义
        }
    }
}

/// 自适应超时策略 —— 按节点自身历史 RTT 推导探测超时
///
/// # 为什么不能用固定值
///
/// 全局固定超时无法区分「节点已死」与「节点天生慢」。实测本机 1756 条
/// 真实样本的延迟分布：
///
/// ```text
/// p50=802  p75=1020  p90=1402  p95=1605  p99=2390  max=2975
/// ```
///
/// 在这个分布上，固定值必然二选一失败（按样本统计误杀率）：
///
/// | 固定超时 | 误杀健康节点 |
/// |---|---|
/// | 1500ms | 6.4% |
/// | 2000ms | 2.3% |
/// | 2500ms | 0.6% |
/// | 3000ms | 0%（旧值） |
///
/// 取 3000ms 虽不误杀，却让**每个高速节点**的故障检测白白多等 2 秒——
/// 而高速节点恰恰是最该被快速发现失效的那批。取 1500ms 则把慢节点反复误杀，
/// 触发本不该发生的故障转移。
///
/// # 策略
///
/// ```text
/// timeout = clamp(baseline × 3, 2000ms, 6000ms)
/// ```
///
/// - 高速节点（baseline 300ms）→ 落到 2000ms 下限，失效判定快
/// - 慢速节点（baseline 2000ms）→ 落到 6000ms 上限，不被自身速度误杀
/// - 无历史的新节点 → 用全局 EWMA 先验（冷启动时回退 1500ms ≈ p90）
/// - 连续失败时收紧 1 倍：失效节点往往 TCP 直接被拒（RTT≈0），不必等满余量；
///   收紧封顶 1 倍（不降到 floor 以下），避免偶发抖动被过度解读
///
/// 3 倍余量的依据：3×p50≈2400ms 已覆盖 p95（1605ms）之上的全部抖动；
/// 再放大只会拖慢故障检测。
#[derive(Debug, Clone)]
pub struct AdaptiveTimeout {
    /// 无节点历史、无全局样本时的回退基准（毫秒）
    pub default_baseline_ms: u32,
    /// 全局 RTT 指数滑动均值（跨节点共享，作为新节点先验）
    global_ewma_ms: Arc<AtomicU32>,
    /// 余量倍数
    pub slack: u32,
    /// 下限（毫秒）：再快的节点也不低于此，避免抖动误判
    pub floor_ms: u64,
    /// 上限（毫秒）：再慢的节点也不超过此，否则故障检测失去意义
    pub ceil_ms: u64,
}

impl Default for AdaptiveTimeout {
    fn default() -> Self {
        Self {
            default_baseline_ms: 1500, // 实测 p90=1402，取整留余量
            global_ewma_ms: Arc::new(AtomicU32::new(0)),
            slack: 3,
            floor_ms: 2000,
            ceil_ms: 6000,
        }
    }
}

impl AdaptiveTimeout {
    /// 覆盖超时上下界（供上层按配置或测试调整）
    ///
    /// 用 builder 而非结构体更新语法：`global_ewma_ms` 刻意保持私有
    /// （它是跨节点共享的运行时状态，不该被外部随手重置）。
    pub fn with_bounds(mut self, floor_ms: u64, ceil_ms: u64) -> Self {
        let floor = floor_ms.max(1);
        self.floor_ms = floor;
        // ceil 必须 ≥ floor，否则 clamp 会产出倒挂区间
        self.ceil_ms = ceil_ms.max(floor);
        self
    }

    /// 记录一次成功 RTT，更新全局先验（EWMA）
    ///
    /// 权重取 **1/16** 而非常见的 1/4：全局先验只服务于「无历史的新节点」，
    /// 它错了会让一批新节点集体用错超时，故必须**极难被单次观测带偏**。
    ///
    /// 实测对比（先验 800ms，来了一个 9000ms 的异常样本）：
    /// ```text
    /// α=1/4  → 2850ms（+256%，单次异常把先验推高 3.5 倍）
    /// α=1/8  → 1825ms（+128%）
    /// α=1/16 → 1312ms（+64%）
    /// ```
    /// 再叠加 `max_step` 上限：任何单次观测最多把先验抬高 1/3，杜绝离群值主导。
    /// 代价是收敛慢（需约 30~50 个样本才跟上真实分布），但先验本身只需
    /// 「大致正确」——真正的精度由各节点自己的 `rtt_ms` 提供。
    pub fn observe(&self, rtt_ms: u16) {
        let prev = self.global_ewma_ms.load(Ordering::Relaxed);
        let next = if prev == 0 {
            rtt_ms as u32
        } else {
            // α=1/16，再以「单次最多抬升 1/3」封顶，杜绝离群值主导
            let ewma = prev.saturating_mul(15).saturating_add(rtt_ms as u32) / 16;
            let cap = prev.saturating_add(prev / 3).max(1);
            ewma.min(cap)
        };
        self.global_ewma_ms.store(next, Ordering::Relaxed);
    }

    /// 全局先验（0 = 尚无任何样本）
    pub fn global_baseline_ms(&self) -> u32 {
        self.global_ewma_ms.load(Ordering::Relaxed)
    }

    /// 计算某节点本次探测的超时（毫秒）
    ///
    /// `node` 的延迟是该节点自己的最近一次成功值——它反映「这条线路有多快」，
    /// 比全局统计更能预测「它现在还该不该在这个时间内回应」。
    pub fn timeout_ms_for(&self, node: Option<&NodeProbe>) -> u64 {
        let baseline = node
            .and_then(|n| n.rtt_ms)
            .map(|r| r as u32)
            .or_else(|| {
                let g = self.global_baseline_ms();
                if g > 0 {
                    Some(g)
                } else {
                    None
                }
            })
            .unwrap_or(self.default_baseline_ms);

        // 连续失败时收紧：失效节点通常立即失败（连接被拒），无需等满余量。
        // 封顶 1 倍——收到 floor 保护，且不把偶发抖动误判为快速失效。
        let shrink = 1u32 << node.map(|n| n.fail_streak).unwrap_or(0).min(1);
        let raw = baseline.saturating_mul(self.slack) / shrink;
        (raw as u64).clamp(self.floor_ms, self.ceil_ms)
    }
}

/// 单节点探测态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeProbe {
    pub tag: String,
    /// 最近一次成功延迟（毫秒）。None = 从未成功或已失效
    pub rtt_ms: Option<u16>,
    /// 最近一次成功的时间戳（毫秒）
    pub last_ok_at: Option<i64>,
    /// 最近一次探测失败的时间戳（毫秒）；故障转移的判据。
    /// 与 `last_ok_at` 比较可知「最近一次探测究竟成功还是失败」。
    pub last_failed_at: Option<i64>,
    /// 连续失败次数
    pub fail_streak: u32,
    pub tier: ProbeTier,
    /// 下次应探测的时间戳（毫秒，含抖动）
    pub next_due_at: i64,
    /// 分层间隔基准（用户可配；随节点走以支持按组差异化）
    pub intervals: TierIntervals,
}

impl NodeProbe {
    pub fn new(tag: &str, now: i64) -> Self {
        Self::with_intervals(tag, now, TierIntervals::default())
    }

    /// 指定分层间隔构造
    pub fn with_intervals(tag: &str, now: i64, intervals: TierIntervals) -> Self {
        Self {
            tag: tag.to_string(),
            rtt_ms: None,
            last_ok_at: None,
            last_failed_at: None,
            fail_streak: 0,
            tier: ProbeTier::Standby,
            // 首轮抖动：避免启动瞬间 370 个节点同时开火（这正是旧架构的脉冲）
            next_due_at: now
                + Self::jitter_offset(tag, intervals.standby_ms),
            intervals,
        }
    }

    /// 本层基准间隔（读用户覆盖，非硬编码）
    pub fn base_interval_ms(&self) -> i64 {
        match self.tier {
            ProbeTier::Active => self.intervals.active_ms,
            ProbeTier::Standby => self.intervals.standby_ms,
            ProbeTier::Cooling => self.intervals.cooling_ms,
        }
    }

    /// 哈希抖动偏移（0..interval 毫秒）
    ///
    /// 用 tag 哈希而非随机数：同一节点每次重启的相位稳定，探测节奏可预期；
    /// 不同节点相位散开，把 370 次探测摊平到整个间隔窗口而非挤在 t=0。
    pub fn jitter_offset(tag: &str, interval_ms: i64) -> i64 {
        if interval_ms <= 0 {
            return 0;
        }
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in tag.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        (h % interval_ms as u64) as i64
    }

    /// 是否已到探测时间
    pub fn is_due(&self, now: i64) -> bool {
        now >= self.next_due_at
    }

    /// 记录一次成功
    pub fn record_success(&mut self, rtt_ms: u16, now: i64) {
        self.rtt_ms = Some(rtt_ms);
        self.last_ok_at = Some(now);
        self.fail_streak = 0;
        // 成功即脱离 Cooling
        if self.tier == ProbeTier::Cooling {
            self.tier = ProbeTier::Standby;
        }
        self.schedule_next(now, None);
    }

    /// 记录一次「主实例实测」延迟（健康守卫回写）
    ///
    /// 与 `record_success` 的区别：这是**主实例**（用户真实路径）测得的延迟，
    /// 比探测面（独立 test-core 进程）更权威——同一进程、同一连接资源。
    /// 顺带把下次探测推后一个间隔，免除探测面对该节点的重复测量。
    pub fn record_observed(&mut self, rtt_ms: u16, now: i64) {
        self.rtt_ms = Some(rtt_ms);
        self.last_ok_at = Some(now);
        self.fail_streak = 0;
        self.tier = ProbeTier::Active;
        self.next_due_at = now + self.base_interval_ms();
    }

    /// 记录一次失败（按分类退避）
    ///
    /// **立即作废该节点的延迟**：`rtt_ms` / `last_ok_at` 必须清空。保留旧值
    /// 会造成一个致命后果——已断线的节点仍以「上次的成功延迟」参与最优评选，
    /// 持续被选为最优，用户因此卡死在死节点上（实测复现：节点 A 连续两次
    /// 失败后 `best_of` 仍返回 A，故障转移完全不触发）。
    ///
    /// 迟滞只应作用于「两个都健康的节点之间比谁快」，绝不能凌驾于
    /// 「当前节点是否还活着」之上——后者是可用性问题，前者是优化问题。
    ///
    /// **Active 层例外**：当前正在使用的节点失败时**不退避**（见
    /// [`Self::pin_active`]）——退避会让故障转移延迟随失败次数指数增长。
    pub fn record_failure(&mut self, class: FailureClass, now: i64) {
        // 作废旧延迟：失败即视为「当前不可用」
        self.rtt_ms = None;
        self.last_ok_at = None;
        // 显式记录「最近一次探测失败」——故障转移的判据。
        // 不能靠 rtt_ms.is_none() 推断：从未探测过的新节点同样是 None，
        // 会被误判为失效而触发无谓切换。
        self.last_failed_at = Some(now);

        if self.tier == ProbeTier::Active {
            // 当前节点：不退避，60s 后立刻复探。
            // 退避会让故障转移延迟随失败次数指数增长（第 3 次失败要等
            // 2 分钟才重新确认，用户已经断了很久）。
            self.next_due_at = now + self.intervals.active_ms;
            return;
        }

        self.fail_streak = self.fail_streak.saturating_add(1);
        // 连续失败即降为 Cooling（成功时自动回升 Standby）
        self.tier = ProbeTier::Cooling;
        self.schedule_next(now, Some(class));
    }

    /// 最近一次探测是否失败过（故障转移判据）
    ///
    /// 只认「探过且失败」，不认「从未探过」——后者是信息缺失而非失效。
    pub fn last_probe_failed(&self) -> bool {
        match (self.last_failed_at, self.last_ok_at) {
            (Some(f), o) => o.map_or(true, |ok| f > ok),
            (None, _) => false,
        }
    }

    /// 计算下次探测时间（基准间隔 × 退避倍数 × 抖动）
    fn schedule_next(&mut self, now: i64, class: Option<FailureClass>) {
        let base = self.base_interval_ms();
        // 指数退避：30s → 60s → 120s ... 上限 1h；再按失败分类加速
        let shift = self.fail_streak.min(7); // 2^7=128，30s×128≈64min，由 min 钳到 1h
        let weight = class.map(|c| c.backoff_weight()).unwrap_or(1) as i64;
        // 抖动只作用于「基准间隔」而非「退避后总量」：否则大 jitter 可能让
        // 第 N 次失败的下次探测时间早于第 N-1 次，退避形同虚设（单测已捕获）。
        // 这样 base 仍是退避下界，offset 落在 [base, 2*base)，
        // 退避倍数增长时 offset 必然同步增长 —— 单调不减得以保持。
        let jitter_span = base;
        let offset = base
            .saturating_add(Self::jitter_offset(&self.tag, jitter_span))
            .saturating_mul(1i64 << shift)
            .saturating_mul(weight)
            .min(3_600_000);
        self.next_due_at = now + offset;
    }

    /// 有效延迟：仅当结果新鲜（`max_age_ms` 内成功过）才返回
    ///
    /// 陈旧的"最优"比没有最优更危险——按陈旧数据切换会把用户切到已下线的节点。
    pub fn effective_rtt(&self, now: i64, max_age_ms: i64) -> Option<u16> {
        let last = self.last_ok_at?;
        if now - last > max_age_ms {
            return None;
        }
        self.rtt_ms
    }
}



/// 节点延迟表
#[derive(Debug, Default)]
pub struct ProbeTable {
    nodes: HashMap<String, NodeProbe>,
    intervals: TierIntervals,
}

impl ProbeTable {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            intervals: TierIntervals::default(),
        }
    }

    /// 设置分层间隔（用户配置的 interval 入口）
    ///
    /// 只影响**尚未进入退避**的节点；已退避的节点按自身节奏走完，
    /// 避免用户改设置后立刻打断既有退避（那会造成一轮探测突发）。
    pub fn set_intervals(&mut self, intervals: TierIntervals) {
        self.intervals = intervals;
        for n in self.nodes.values_mut() {
            n.intervals = intervals;
        }
    }

    pub fn intervals(&self) -> TierIntervals {
        self.intervals
    }

    /// 全量重设节点集（订阅刷新后调用）
    ///
    /// 保留仍在集合内的节点历史（避免刷新订阅导致全部节点重回"未测"，
    /// 那会让启动瞬间 370 个节点同时成为待探测项——重新制造脉冲）。
    pub fn reset_to(&mut self, tags: &[String], now: i64) {
        let incoming: std::collections::HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();
        // 移除已消失的节点
        self.nodes.retain(|tag, _| incoming.contains(tag.as_str()));
        // 新增节点（已存在的保留原状态）
        for tag in tags {
            self.nodes
                .entry(tag.clone())
                .or_insert_with(|| NodeProbe::with_intervals(tag, now, self.intervals));
        }
    }

    pub fn get(&self, tag: &str) -> Option<&NodeProbe> {
        self.nodes.get(tag)
    }

    pub fn get_mut(&mut self, tag: &str) -> Option<&mut NodeProbe> {
        self.nodes.get_mut(tag)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 迭代全部节点
    pub fn iter(&self) -> impl Iterator<Item = &NodeProbe> {
        self.nodes.values()
    }

    /// 取出当前到期的节点 tag（并顺带把它们推后，防止同轮重复派发）
    pub fn take_due(&mut self, now: i64, max_count: usize) -> Vec<String> {
        let mut due: Vec<String> = self
            .nodes
            .values()
            .filter(|n| n.is_due(now))
            .map(|n| n.tag.clone())
            .collect();
        due.sort();
        due.truncate(max_count);
        // 派发后推后，避免本轮内被重复选中。
        // 用完整基准间隔（而非 /4）：/4 会让刚失败的节点 7 秒后又被重探，
        // 与退避设计（30s 起指数增长）相冲突。
        for tag in &due {
            if let Some(n) = self.nodes.get_mut(tag) {
                n.next_due_at = now + n.base_interval_ms();
            }
        }
        due
    }

    /// 将指定节点提升为 Active 层（当前选中 / pinned）
    pub fn promote(&mut self, tags: &[String], now: i64) {
        for tag in tags {
            if let Some(n) = self.nodes.get_mut(tag) {
                n.tier = ProbeTier::Active;
                n.schedule_next(now, None);
            }
        }
    }

    /// 标记「内核当前正在使用的节点」：提为 Active、立即到期、且锁定不退避
    ///
    /// 与 `promote` 的区别：这里把 `next_due_at` 直接设为 now（立即探测），
    /// 并清零 `fail_streak`——当前节点必须是故障转移信号的**首个**来源，
    /// 不能因为之前失败过就被推到退避队列末尾。
    pub fn pin_active(&mut self, tag: &str, now: i64) {
        if let Some(n) = self.nodes.get_mut(tag) {
            n.tier = ProbeTier::Active;
            n.fail_streak = 0;
            n.next_due_at = now;
        }
    }

    /// 选出最优节点：在成员中挑延迟最低且结果新鲜的
    ///
    /// 返回 `(tag, rtt_ms)`。`max_age_ms` 之外的结果视为无效（见
    /// [`NodeProbe::effective_rtt`]）。同延迟时按 tag 字典序取小者，保证
    /// 结果稳定可复现（不引入随机切换）。
    pub fn best_of(&self, members: &[String], now: i64, max_age_ms: i64) -> Option<(String, u16)> {
        members
            .iter()
            .filter_map(|tag| {
                self.nodes
                    .get(tag)
                    .and_then(|n| n.effective_rtt(now, max_age_ms))
                    .map(|rtt| (tag.clone(), rtt))
            })
            .min_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
    }

    /// 健康节点统计：(结果新鲜的节点数, 总节点数)
    pub fn health_summary(&self, now: i64, max_age_ms: i64) -> (usize, usize) {
        let healthy = self
            .nodes
            .values()
            .filter(|n| n.effective_rtt(now, max_age_ms).is_some())
            .count();
        (healthy, self.nodes.len())
    }

    /// 从持久化记录回填历史（启动时调用）
    ///
    /// 与 `reset_to` 的分工：先 `reset_to` 建立当前节点集，再 `restore_from`
    /// 填回历史。**只回填仍然新鲜的记录**——回填陈旧延迟会让启动首轮就按
    /// 过期数据选点，把用户切到已下线的节点。
    pub fn restore_from(&mut self, records: &[crate::core::stats_db::ProbeStateRecord], now: i64, max_age_ms: i64) {
        for r in records {
            let Some(n) = self.nodes.get_mut(&r.node_tag) else {
                continue; // 节点已随订阅刷新消失
            };
            n.rtt_ms = r.rtt_ms;
            n.last_ok_at = r.last_ok_at;
            n.fail_streak = r.fail_streak;
            // 仍然新鲜的失败计数保留（继续退避），过期记录视为无效
            if n
                .last_ok_at
                .map(|t| now - t > max_age_ms)
                .unwrap_or(true)
            {
                n.rtt_ms = None;
                n.last_ok_at = None;
            }
            if n.fail_streak > 0 {
                n.tier = ProbeTier::Cooling;
            }
        }
    }

    /// 导出当前全部状态（落盘用）
    pub fn snapshot(&self) -> Vec<crate::core::stats_db::ProbeStateRecord> {
        self.nodes
            .values()
            .map(|n| crate::core::stats_db::ProbeStateRecord {
                node_tag: n.tag.clone(),
                rtt_ms: n.rtt_ms,
                last_ok_at: n.last_ok_at,
                fail_streak: n.fail_streak,
            })
            .collect()
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn tags(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("node-{i:03}")).collect()
    }

    #[test]
    fn jitter_spreads_nodes_across_the_window() {
        // 核心不变量：370 节点不得挤在同一时刻开火（旧架构的探测脉冲）
        let ts = tags(370);
        let window = ProbeTier::Standby.base_interval_ms();
        let offsets: Vec<i64> = ts
            .iter()
            .map(|t| NodeProbe::jitter_offset(t, window))
            .collect();
        // 全部落在窗口内
        assert!(offsets.iter().all(|&o| o >= 0 && o < window));
        // 分布分散：10 个等宽桶，每桶都应有节点（否则说明相位聚集）
        let mut buckets = [0usize; 10];
        for &o in &offsets {
            let idx = ((o as f64 / window as f64) * 10.0) as usize;
            buckets[idx.min(9)] += 1;
        }
        assert!(
            buckets.iter().all(|&c| c > 0),
            "抖动未分散，桶分布 {:?}",
            buckets
        );
    }

    #[test]
    fn jitter_is_deterministic_across_restarts() {
        // 相位稳定：同一节点每次重启抖动一致，探测节奏可预期
        assert_eq!(
            NodeProbe::jitter_offset("node-001", 300_000),
            NodeProbe::jitter_offset("node-001", 300_000)
        );
        assert_eq!(NodeProbe::jitter_offset("x", 0), 0);
        assert_eq!(NodeProbe::jitter_offset("x", -5), 0);
    }

    #[test]
    fn backoff_grows_monotonically_and_is_capped() {
        let mut n = NodeProbe::new("a", NOW);
        let mut last_gap = 0i64;
        for i in 0..10 {
            n.record_failure(FailureClass::Timeout, NOW);
            let gap = n.next_due_at - NOW;
            assert!(gap > 0, "第 {} 次失败后间隔必须为正", i + 1);
            assert!(gap <= 3_600_000, "退避不得超过 1h，实际 {}ms", gap);
            if i > 0 {
                assert!(gap >= last_gap, "退避必须单调不减");
            }
            last_gap = gap;
        }
        assert!(last_gap > 1_800_000, "多次失败后应退避到半小时以上");
    }

    #[test]
    fn failure_class_weight_orders_backoff() {
        // 配置错(Dns)退避应快于瞬态(Tls)
        let mut tls = NodeProbe::new("a", NOW);
        let mut dns = NodeProbe::new("a", NOW);
        tls.record_failure(FailureClass::Tls, NOW);
        dns.record_failure(FailureClass::Dns, NOW);
        assert!(dns.next_due_at > tls.next_due_at);

        // 唯一值得立即重试的是 TLS（旧实现对全部失败重试）
        assert!(FailureClass::Tls.worth_retry_now());
        for c in [
            FailureClass::Timeout,
            FailureClass::Unreachable,
            FailureClass::Dns,
            FailureClass::Unknown,
        ] {
            assert!(!c.worth_retry_now(), "{:?} 不应立即重试", c);
        }
    }

    #[test]
    fn success_resets_streak_and_lifts_cooling() {
        let mut n = NodeProbe::new("a", NOW);
        n.record_failure(FailureClass::Timeout, NOW);
        n.record_failure(FailureClass::Timeout, NOW);
        assert_eq!(n.fail_streak, 2);
        n.record_success(250, NOW);
        assert_eq!(n.fail_streak, 0);
        assert_eq!(n.tier, ProbeTier::Standby, "成功后应脱离 Cooling");
        assert_eq!(n.rtt_ms, Some(250));
        assert_eq!(n.last_ok_at, Some(NOW));
    }

    #[test]
    fn stale_results_are_not_effective() {
        // 陈旧的"最优"比没有最优更危险：会把用户切到已下线节点
        let mut n = NodeProbe::new("a", NOW);
        n.record_success(200, NOW);
        assert_eq!(n.effective_rtt(NOW + 60_000, 120_000), Some(200));
        assert_eq!(n.effective_rtt(NOW + 200_000, 120_000), None);
    }

    #[test]
    fn take_due_is_deterministic_and_does_not_repeat() {
        let mut t = ProbeTable::new();
        let ts = tags(50);
        t.reset_to(&ts, NOW);
        for tag in &ts {
            t.get_mut(tag).unwrap().next_due_at = NOW;
        }
        let first = t.take_due(NOW, 10);
        assert_eq!(first.len(), 10);
        let mut sorted = first.clone();
        sorted.sort();
        assert_eq!(first, sorted, "派发顺序必须稳定可复现");
        let second = t.take_due(NOW, 10);
        assert!(
            !second.iter().any(|t2| first.contains(t2)),
            "同轮重复派发：{:?}",
            second
        );
    }

    #[test]
    fn reset_to_preserves_history_of_surviving_nodes() {
        // 订阅刷新不得让全部节点重回"未测"——那会重新制造启动脉冲
        let mut t = ProbeTable::new();
        t.reset_to(&tags(370), NOW);
        t.get_mut("node-000").unwrap().record_success(180, NOW);

        let mut after = tags(360);
        for i in 0..10 {
            after.push(format!("fresh-{i}"));
        }
        t.reset_to(&after, NOW);

        assert_eq!(
            t.get("node-000").unwrap().rtt_ms,
            Some(180),
            "存活节点历史应保留"
        );
        assert!(t.get("node-369").is_none(), "消失节点应被移除");
        assert!(t.get("fresh-0").is_some(), "新增节点应入表");
        assert_eq!(t.len(), 370);
    }

    #[test]
    fn best_of_ignores_stale_and_is_stable_on_ties() {
        let mut t = ProbeTable::new();
        let ts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        t.reset_to(&ts, NOW);
        t.get_mut("a").unwrap().record_success(300, NOW);
        t.get_mut("b").unwrap().record_success(200, NOW);
        t.get_mut("c").unwrap().record_success(200, NOW);

        let best = t.best_of(&ts, NOW, 120_000).unwrap();
        assert_eq!(best.1, 200);
        assert_eq!(best.0, "b", "同延迟应按 tag 字典序稳定选取");
        // 全部过期 → 无最优（宁可不动，也不要切到陈旧节点）
        assert!(t.best_of(&ts, NOW + 500_000, 120_000).is_none());
    }

    #[test]
    fn health_summary_counts_only_fresh_successes() {
        let mut t = ProbeTable::new();
        let ts = tags(10);
        t.reset_to(&ts, NOW);
        for tag in ts.iter().take(4) {
            t.get_mut(tag).unwrap().record_success(150, NOW);
        }
        assert_eq!(t.health_summary(NOW, 120_000), (4, 10));
        assert_eq!(t.health_summary(NOW + 500_000, 120_000).0, 0);
    }

    #[test]
    fn promote_marks_active_and_shortens_interval() {
        let mut t = ProbeTable::new();
        let ts = tags(3);
        t.reset_to(&ts, NOW);
        t.promote(&["node-000".to_string()], NOW);
        assert_eq!(t.get("node-000").unwrap().tier, ProbeTier::Active);
        assert_eq!(t.get("node-001").unwrap().tier, ProbeTier::Standby);
        let a = t.get("node-000").unwrap().next_due_at - NOW;
        // 抖动作用于基准间隔 → offset 落在 [base, 2*base)
        let base = ProbeTier::Active.base_interval_ms();
        assert!(
            a >= base && a < 2 * base,
            "Active 下次探测应落在 [{base}, {}), 实际 {a}",
            2 * base
        );
    }
}

#[cfg(test)]
mod adaptive_timeout_tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn node_with_rtt(rtt: Option<u16>) -> NodeProbe {
        let mut n = NodeProbe::new("x", NOW);
        n.rtt_ms = rtt;
        n
    }

    /// 实测分布（1756 样本）：p50=802 p95=1605 p99=2390 max=2975
    /// 自适应超时的核心价值：快节点不必等满上界
    #[test]
    fn fast_node_gets_far_shorter_timeout_than_slow_node() {
        let a = AdaptiveTimeout::default();
        // 高速节点（p50 量级）
        let fast = a.timeout_ms_for(Some(&node_with_rtt(Some(300))));
        // 慢速节点（p99 量级）
        let slow = a.timeout_ms_for(Some(&node_with_rtt(Some(2400))));
        assert_eq!(fast, 2_000, "高速节点应落到 floor");
        assert_eq!(slow, 6_000, "慢速节点应落到 ceil");
        assert!(
            slow > fast * 2,
            "自适应必须显著区分快慢节点（{slow} vs {fast}）"
        );
    }

    /// 相对固定 5000ms 的收益：高速节点的故障检测快一倍以上
    #[test]
    fn adaptive_beats_fixed_ceiling_for_most_nodes() {
        let a = AdaptiveTimeout::default();
        let fixed = 5_000u64;
        // 典型分布下多数节点应显著快于固定上界
        let p50_node = a.timeout_ms_for(Some(&node_with_rtt(Some(802))));
        assert!(
            p50_node < fixed,
            "p50 节点的探测超时应短于固定上界：{p50_node} vs {fixed}"
        );
    }

    /// 无历史的冷启动节点：不得因「拿不到基线」而使用过紧的超时
    #[test]
    fn cold_start_node_uses_safe_default() {
        let a = AdaptiveTimeout::default();
        let t = a.timeout_ms_for(None);
        // 无基线时至少要有 floor 保护，不能因缺数据就误杀
        assert!(t >= a.floor_ms, "冷启动超时 {t} 低于 floor {}", a.floor_ms);
    }

    /// 全局 EWMA 先验：无历史节点应继承其他节点的经验，而非一律用兜底值
    #[test]
    fn global_ewma_acts_as_prior_for_new_nodes() {
        let a = AdaptiveTimeout::default();
        assert_eq!(a.global_baseline_ms(), 0, "初始无样本");
        let t_before = a.timeout_ms_for(None);
        a.observe(2400); // 观测到一个慢节点
        assert_eq!(a.global_baseline_ms(), 2400);
        let t_after = a.timeout_ms_for(None);
        assert!(
            t_after >= t_before,
            "先验变慢后超时不应收紧：{t_before} → {t_after}"
        );
    }

    /// EWMA 抗抖动：单次极慢观测不应把全局先验带偏太多
    #[test]
    fn ewma_resists_single_outlier() {
        let a = AdaptiveTimeout::default();
        for _ in 0..10 {
            a.observe(800);
        }
        let before = a.global_baseline_ms();
        a.observe(9000); // 一次异常
        let after = a.global_baseline_ms();
        // α=1/4 → 单次异常最多拉高 25%
        assert!(
            after <= before + (before as u32) / 3,
            "单次异常把先验从 {before} 拉到 {after}，抗抖动不足"
        );
    }

    /// 连续失败时收紧：失效节点通常立即失败（连接被拒），无需等满余量
    #[test]
    fn consecutive_failure_shrinks_timeout() {
        let a = AdaptiveTimeout::default();
        let mut n = node_with_rtt(Some(1500));
        let normal = a.timeout_ms_for(Some(&n));
        n.fail_streak = 1;
        let shrunk = a.timeout_ms_for(Some(&n));
        assert!(
            shrunk <= normal,
            "失败后超时应收紧以加速判定：{normal} → {shrunk}"
        );
    }

    /// 边界：floor/ceil 配置不得产生倒挂区间
    #[test]
    fn bounds_never_invert() {
        let a = AdaptiveTimeout::default().with_bounds(5_000, 1_000);
        assert!(a.ceil_ms >= a.floor_ms, "ceil 必须 >= floor");
        let t = a.timeout_ms_for(Some(&node_with_rtt(Some(800))));
        assert!(t >= a.floor_ms && t <= a.ceil_ms, "实际值 {t} 越界");
    }
}
