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

/// 节点延迟画像 —— 区分「天生慢」与「抖动大」
///
/// # 全部使用相对量纲，不含任何绝对毫秒阈值
///
/// 早期版本用「中位 > 1500ms 算慢」「floor = 2000ms」这类**绝对阈值**，
/// 那是照着某一个机场的样本反推的，直接换订阅就失效：
///
/// ```text
/// 欧洲订阅（median 50ms）→ floor 2000ms 成了唯一约束，
///                          50ms 的节点也要等 2×2000=4s 才判死
/// 跨太平洋专线（median 800ms 稳定）→ 3×800=2400ms 勉强够，
///                          但若该线路 p50=1200ms 就逼近 ceil
/// ```
///
/// 故此处只用**比值**：节点自身的离散度、以及它与全体节点中位数的相对位置。
/// 绝对毫秒值一律由观测数据推导（fleet median），不写死。
///
/// # 两个关键量
///
/// - `jitter` = (p95 − median) / median：该节点自身的波动幅度。**无量纲**，
///   与机场无关
/// - `rel_slow` = median / fleet_median：该节点比同订阅其他节点慢多少倍。
///   **无量纲**，自动适配「整体很快」或「整体很慢」的订阅
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LatencyProfile {
    /// 样本不足，无法判断
    Unknown,
    /// 与全体节点同步（既不特别慢也不特别抖）
    Typical,
    /// 明显比同订阅其他节点慢，但自身稳定 → 天生慢线路
    RelativelySlow,
    /// 自身波动大 → 抖动
    Jittery,
}

impl LatencyProfile {
    /// 判定所需的最少样本
    const MIN_SAMPLES: usize = 4;
    /// 相对全体中位数慢到此倍数以上即判「天生慢」（2× = 明显更慢）
    const REL_SLOW_RATIO: f64 = 2.0;
    /// 自身离散度超过此值即判「抖动」（p95 达中位数的 2 倍）
    const JITTER_RATIO: f64 = 1.0;

    /// 由样本 + 全体中位数判定画像
    ///
    /// `fleet_median_ms` 为全体节点成功延迟的中位数（0 = 尚无全局样本）。
    pub fn classify(samples: &[u16], fleet_median_ms: u64) -> Self {
        if samples.len() < Self::MIN_SAMPLES {
            return LatencyProfile::Unknown;
        }
        let mut v: Vec<u64> = samples.iter().map(|s| *s as u64).collect();
        v.sort_unstable();
        let median = v[v.len() / 2] as f64;
        if median <= 0.0 {
            return LatencyProfile::Unknown;
        }
        let p95 = v[((v.len() as f64) * 0.95).min(v.len() as f64 - 1.0) as usize] as f64;
        // 自身波动：无量纲
        let jitter = (p95 - median) / median;
        // 相对全体：与订阅整体对比，自动适配整体快/慢
        let rel_slow = if fleet_median_ms > 0 {
            median / fleet_median_ms as f64
        } else {
            1.0
        };

        if jitter >= Self::JITTER_RATIO {
            LatencyProfile::Jittery
        } else if fleet_median_ms > 0 && rel_slow >= Self::REL_SLOW_RATIO {
            LatencyProfile::RelativelySlow
        } else {
            LatencyProfile::Typical
        }
    }

    /// 该画像下的余量倍数调整
    ///
    /// - `RelativelySlow`：基线本身就大，按同一倍数算出的超时可能撞上
    ///   `ceil` 而失去区分度，故额外 +1 档
    /// - 其余：基准倍数（抖动已由「按倍数而非按绝对值放宽」天然覆盖）
    pub fn slack_adjust(self, base_slack: u32) -> u32 {
        match self {
            LatencyProfile::RelativelySlow => base_slack.saturating_add(1),
            _ => base_slack,
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
/// # 策略（三层，逐层精确化）
///
/// ```text
/// 样本 < 20   →  timeout = clamp(基线 × 3,  fleet中位×2,  fleet中位×8)
/// 样本 ≥ 20   →  timeout = clamp(经验p95 × 1.2, 同上)
/// 连续失败    →  上述结果再 ÷2（失效节点往往 TCP 直接被拒，不必等满余量）
/// ```
///
/// **上下界是相对量纲而非绝对毫秒数**——这是跨机场自适应的关键。绝对值只对
/// 采样时那一家有效：fleet 50ms（欧洲节点）→ floor 100ms，故障快速判定；
/// fleet 800ms（跨洋专线）→ floor 1600ms，慢节点不被自身速度误杀。
/// 另有 500ms 绝对下限兜底「fleet 极快时 floor 退化到毫秒级」的极端情况。
///
/// **经验分位数替代固定倍数**：实测 203 节点中 `3×median / 经验p95` 的
/// 中位数为 1.78——固定 3 倍对 87% 的节点都过于宽松，等于白等：
/// 巴西节点 p95=1371ms 却要等 2682ms 才判失效。样本累积到 20+ 后自动切换，
/// 用户无需配置任何参数。
///
/// 门槛为何是 20 而非更小：经验分位数的分辨率由样本量决定，n<20 时 p95 会
/// 退化为最大值（n=12 → 索引 11 = max），等于「拿单个离群点当下限」，比固定
/// 倍数更不稳。20 是 p95 首次取得真实分辨率的位置。
#[derive(Debug, Clone)]
pub struct AdaptiveTimeout {
    /// 全局 RTT 指数滑动均值（跨节点共享，作为新节点先验）
    global_ewma_ms: Arc<AtomicU32>,
    /// 余量倍数
    pub slack: u32,
    /// 下限相对 fleet 中位数的倍数：再快的节点也不低于 fleet×此值
    ///
    /// 用倍数而非绝对毫秒，才能同时适配「欧洲 50ms 节点」与
    /// 「跨太平洋 800ms 专线」两类订阅——绝对值只对采样时那一家有效。
    pub floor_ratio: f64,
    /// 上限相对 fleet 中位数的倍数：保证故障检测延迟有上界
    pub ceil_ratio: f64,
    /// 冷启动兜底：连一个样本都没有时用（毫秒）。
    /// 仅在首轮探测前生效，一旦有任意样本即被 fleet 中位数取代。
    pub cold_start_ms: u64,
}

impl Default for AdaptiveTimeout {
    fn default() -> Self {
        Self {
            global_ewma_ms: Arc::new(AtomicU32::new(0)),
            slack: 3,
            // 2× / 8× fleet 中位数：既不为极快节点设过高的绝对下限，
            // 也不给极慢节点无限放宽
            floor_ratio: 2.0,
            ceil_ratio: 8.0,
            cold_start_ms: 5000,
        }
    }
}

impl AdaptiveTimeout {
    /// 覆盖冷启动兜底值（首轮探测前、尚无任何样本时使用）
    ///
    /// 用 builder 而非结构体更新语法：`global_ewma_ms` 刻意保持私有
    /// （它是跨节点共享的运行时状态，不该被外部随手重置）。
    pub fn with_cold_start(mut self, ms: u64) -> Self {
        self.cold_start_ms = ms.max(1);
        self
    }

    /// 由样本经验分位数导出超时上界；样本不足时返回 None
    ///
    /// 要求 ≥ `MIN_SAMPLES_FOR_EMPIRICAL` 个样本：经验分位数的分辨率由样本量
    /// 决定，样本太少时 p95 实际等于最大值（一个离群点就顶满），不可信。
    ///
    /// 用 `p95 × 1.2` 而非 `p99 × k`：p99 需要 20+ 样本才有意义，而 p95 在
    /// 12 个样本时已有 0.6 个样本的分辨率。1.2 倍于 p95 覆盖了「比历史最差
    /// 情况再差一点」的余量，足以吸收新出现的抖动。
    /// 启用经验分位数所需的最小样本数
    ///
    /// 取 20 而非更小的值：经验分位数的**分辨率**由样本量决定，样本过少时
    /// p95 会退化为「最大值」（n=12 时 p95 索引 11 = max；n<20 基本必然如此），
    /// 那等价于「用最大值当下限」——一个离群样本就能把超时顶满，比固定倍数
    /// 还不稳。20 是 p95 首次获得真实分辨率的位置（n=20 → 索引 18，非 max）。
    const MIN_SAMPLES_FOR_EMPIRICAL: usize = 20;
    const EMPIRICAL_MARGIN_NUM: u64 = 6; // 1.2 = 6/5
    const EMPIRICAL_MARGIN_DEN: u64 = 5;

    fn empirical_ceiling(samples: &[u16]) -> Option<u64> {
        if samples.len() < Self::MIN_SAMPLES_FOR_EMPIRICAL {
            return None;
        }
        let mut v: Vec<u64> = samples.iter().map(|s| *s as u64).collect();
        v.sort_unstable();
        // p95 位置：向上取整，保证「至少覆盖 95% 的观测」
        let idx = ((v.len() as u64 * 95).div_ceil(100) as usize).clamp(1, v.len()) - 1;
        let p95 = v[idx];
        Some(p95 * Self::EMPIRICAL_MARGIN_NUM / Self::EMPIRICAL_MARGIN_DEN)
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
        self.timeout_ms_for_with(node, None)
    }

    /// 画像感知版：传入该节点近期的成功延迟样本后自动选择余量
    ///
    /// 样本不足时画像为 `Unknown`，用基准倍数——此时**无法**区分「天生慢」
    /// 与「抖动」，保守起见不额外放宽。这是有意的：宁可少优化，也不能在
    /// 信息不足时做出错误判断。
    pub fn timeout_ms_for_with(
        &self,
        node: Option<&NodeProbe>,
        samples: Option<&[u16]>,
    ) -> u64 {
        let samples = samples.unwrap_or(&[]);
        let profile = LatencyProfile::classify(samples, self.global_baseline_ms() as u64);
        // 画像修正余量：相对慢的节点基线本就大，需成比例放宽
        let slack = profile.slack_adjust(self.slack);

        // 样本充分时**改用经验分位数**替代固定倍数——这是「用得越久越准」的落点。
        //
        // 固定 3×median 对多数节点是过度宽松：实测 203 节点中
        // `3×median / 经验p95` 的中位数为 1.78（即余量比实际需要宽 78%），
        // 87% 的节点都能收紧。巴西节点 p95=1371ms 却要等 2682ms 才发现失效。
        //
        // 改用 `p95 × 1.2` 后：既覆盖历史观测到的全部抖动（1.2 倍于 p95），
        // 又不必为一个罕见离群点付出 3 倍代价。样本越少越接近经验分位数，
        // 故冷启动期（小样本）仍走固定倍数——那时经验分位数不可信。
        let empirical = Self::empirical_ceiling(samples);
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
            .unwrap_or(self.cold_start_ms as u32);

        // 上下界由 **fleet 中位数 × 比例** 导出，而非写死毫秒数。
        // 这是跨机场自适应的关键：fleet 50ms → floor 100ms（快节点快速判定）；
        // fleet 800ms → floor 1600ms（慢节点不会被自身速度误杀）。
        // 500ms 绝对下限兜底「fleet 极快时 floor 退化到毫秒级」的极端情况。
        let anchor = if self.global_baseline_ms() > 0 {
            self.global_baseline_ms() as f64
        } else {
            // 冷启动：用当前节点自身基线当锚，避免全部退化为兜底值
            baseline as f64
        };
        let floor = (anchor * self.floor_ratio).max(500.0);
        // `RelativelySlow` 已证实在「稳定地慢」，多等一会远小于误杀可用节点
        let ceil = match profile {
            LatencyProfile::RelativelySlow => (anchor * self.ceil_ratio * 2.0).max(floor),
            _ => (anchor * self.ceil_ratio).max(floor),
        };

        // 连续失败时收紧：失效节点通常立即失败（连接被拒），无需等满余量。
        // 封顶 1 倍——收到 floor 保护，且不把偶发抖动误判为快速失效。
        // 基准超时：经验分位数优先（样本充分时），否则用「基线 × 余量」
        let anchor_timeout = match empirical {
            Some(t) => t,
            None => baseline.saturating_mul(slack) as u64,
        };

        let shrink = 1u32 << node.map(|n| n.fail_streak).unwrap_or(0).min(1);
        let raw = anchor_timeout / shrink as u64;
        (raw as f64).clamp(floor, ceil) as u64
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
    /// 近期成功延迟样本（环形缓冲，容量 `RTT_SAMPLE_CAP`）
    ///
    /// 用途：`LatencyProfile` 据此区分「天生慢」与「抖动大」——这是自适应
    /// 超时能自动适配任意机场（而非只适配本机实测数据）的关键。
    /// 只存成功样本：失败节点没有「延迟」可言。
    pub rtt_samples: Vec<u16>,
}

/// RTT 样本环形容量
///
/// 取 32 而非 8：经验分位数的**分辨率**由样本数决定——8 个样本时 p95 的
/// 位置只有 0.4 个样本的分辨率（实际等于最大值），一个离群点就能顶满超时。
/// 实测本机 341 节点的样本数 p50=5 / max=13，从未达到 20，故此前任何基于
/// 分位数的推导都不可靠；容量提到 32 并配合持久化，样本会随使用自然累积。
///
/// 存储代价可接受：370 节点 × 32 × 2B ≈ 24KB（持久化后同一量级）。
pub const RTT_SAMPLE_CAP: usize = 32;

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
            rtt_samples: Vec::with_capacity(RTT_SAMPLE_CAP),
        }
    }

    /// 记入一条 RTT 样本（环形，满则丢弃最旧的）
    fn push_sample(&mut self, rtt_ms: u16) {
        if self.rtt_samples.len() >= RTT_SAMPLE_CAP {
            self.rtt_samples.remove(0);
        }
        self.rtt_samples.push(rtt_ms);
    }

    /// 该节点的延迟画像（`fleet_median_ms` 为全体节点中位数，0 = 尚无全局样本）
    pub fn latency_profile(&self, fleet_median_ms: u64) -> LatencyProfile {
        LatencyProfile::classify(&self.rtt_samples, fleet_median_ms)
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
        self.push_sample(rtt_ms);
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
        self.push_sample(rtt_ms);
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
            // 回填历史样本：经验分位数需跨重启累积，否则用户每次重启都退回
            // 「样本不足」状态，永远无法收敛
            n.rtt_samples = r.rtt_samples.iter().copied().take(RTT_SAMPLE_CAP).collect();
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
                rtt_samples: n.rtt_samples.clone(),
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

    fn node_with(rtt: Option<u16>, samples: &[u16]) -> NodeProbe {
        let mut n = NodeProbe::new("x", NOW);
        n.rtt_ms = rtt;
        n.rtt_samples = samples.to_vec();
        n
    }

    /// 喂样本让全局先验（fleet 中位数代理）收敛到目标值
    fn seeded(target_ms: u16) -> AdaptiveTimeout {
        let a = AdaptiveTimeout::default();
        for _ in 0..64 {
            a.observe(target_ms);
        }
        a
    }

    // ---- 核心：跨机场自适应 ----

    /// 同一个 3×余量，在「快订阅」与「慢订阅」上应给出正比的结果。
    /// 这是相对量纲改造的立论：写死毫秒只会对采样时那一家有效。
    #[test]
    fn timeout_scales_with_fleet_speed_not_hardcoded() {
        let fast = seeded(50); // 欧洲订阅：中位 50ms
        let slow = seeded(800); // 跨太平洋：中位 800ms

        let t_fast = fast.timeout_ms_for(Some(&node_with(Some(50), &[])));
        let t_slow = slow.timeout_ms_for(Some(&node_with(Some(800), &[])));

        assert!(
            t_slow > t_fast * 4,
            "慢订阅的超时应远大于快订阅：{t_slow} vs {t_fast}"
        );
        // 且慢订阅的超时不应被某个写死的上限压平
        assert!(t_slow > 2_000, "慢订阅被硬编码上限压平了：{t_slow}");
    }

    /// 地板：快订阅的节点也必须留抖动余量，但不应像绝对 floor=2000ms 那样
    /// 让 50ms 的节点白等 4 秒。
    #[test]
    fn fast_fleet_gets_small_floor() {
        let a = seeded(50);
        let t = a.timeout_ms_for(Some(&node_with(Some(50), &[])));
        assert!(t < 1_000, "快订阅的 floor 过大：{t}ms（应为 ~2×50=100~500）");
        assert!(t >= 500, "仍需绝对下限防抖：{t}ms");
    }

    /// 冷启动：零样本时用兜底值，但不得崩溃
    #[test]
    fn cold_start_is_safe() {
        let a = AdaptiveTimeout::default();
        let t = a.timeout_ms_for(None);
        assert!(t > 0, "冷启动超时必须为正");
    }

    // ---- 画像：只用相对量纲 ----

    /// 抖动节点：自身 p95 达中位 2 倍
    #[test]
    fn jittery_profile_needs_no_extra_slack() {
        // 中位 800，p95 1600 → jitter=1.0 → Jittery
        let p = LatencyProfile::classify(&[800, 820, 780, 1600, 790], 800);
        assert_eq!(p, LatencyProfile::Jittery);
        // 抖动已由「按倍数放宽」覆盖，无需再加 slack
        assert_eq!(p.slack_adjust(3), 3);
    }

    /// 相对慢：比全体中位数慢 2 倍以上，但自身稳定
    #[test]
    fn relatively_slow_profile_gets_extra_slack() {
        // 中位 1600 稳定，fleet 中位 800 → rel_slow=2.0 → RelativelySlow
        let p = LatencyProfile::classify(&[1600, 1580, 1620, 1610], 800);
        assert_eq!(p, LatencyProfile::RelativelySlow);
        assert_eq!(p.slack_adjust(3), 4, "相对慢的节点需要额外余量");
    }

    /// 同样的绝对延迟，fleet 不同 → 画像不同。这是相对量纲的核心价值。
    #[test]
    fn same_rtt_yields_different_profile_by_fleet() {
        let samples = [1600, 1580, 1620, 1610];
        // 混在大池子里（fleet 800）→ 相对慢
        assert_eq!(
            LatencyProfile::classify(&samples, 800),
            LatencyProfile::RelativelySlow
        );
        // 混在小池子里（fleet 2000）→ 只是典型
        assert_eq!(
            LatencyProfile::classify(&samples, 2000),
            LatencyProfile::Typical
        );
    }

    /// 样本不足时不得妄下判断
    #[test]
    fn insufficient_samples_yield_unknown() {
        assert_eq!(LatencyProfile::classify(&[100, 200], 800), LatencyProfile::Unknown);
        assert_eq!(LatencyProfile::classify(&[], 800), LatencyProfile::Unknown);
    }

    /// 中位快但波动大 → Jittery 优先于 RelativelySlow
    #[test]
    fn jitter_takes_precedence_over_slow() {
        // 中位 1600（慢）但 p95 4000（cv=1.5 抖）→ 抖动是主要矛盾
        let p = LatencyProfile::classify(&[1600, 1610, 1590, 4000, 1605], 800);
        assert_eq!(p, LatencyProfile::Jittery);
    }

    // ---- EWMA 先验抗离群 ----

    #[test]
    fn ewma_resists_single_outlier() {
        let a = AdaptiveTimeout::default();
        for _ in 0..40 {
            a.observe(500);
        }
        let before = a.global_baseline_ms();
        a.observe(9000);
        let after = a.global_baseline_ms();
        assert!(
            after <= before + before / 3,
            "单次异常从 {before} 拉到 {after}，抗抖动不足"
        );
    }

    #[test]
    fn ewma_acts_as_prior_for_new_nodes() {
        let a = seeded(2000);
        let t = a.timeout_ms_for(None);
        assert!(t > 0);
        // 新节点继承全体先验，而非冷启动兜底值
        assert!(t >= 2_000, "新节点未继承先验：{t}");
    }

    // ---- 失败收缩 ----

    #[test]
    fn consecutive_failure_shrinks_timeout() {
        let a = seeded(800);
        let mut n = node_with(Some(800), &[]);
        let normal = a.timeout_ms_for(Some(&n));
        n.fail_streak = 1;
        let shrunk = a.timeout_ms_for(Some(&n));
        assert!(shrunk <= normal, "失败后应收紧：{normal} → {shrunk}");
    }

    /// 相对慢的节点即便连续失败也不得低于地板
    #[test]
    fn shrink_respects_floor() {
        let a = seeded(2000);
        let mut n = node_with(Some(2000), &[]);
        n.fail_streak = 3;
        let t = a.timeout_ms_for(Some(&n));
        assert!(t >= 500, "收缩后跌破地板：{t}");
    }

    /// 样本环容量受控：不会随节点数 × 历史无限膨胀
    #[test]
    fn rtt_samples_ring_is_bounded() {
        let mut n = NodeProbe::new("x", NOW);
        for i in 0..(RTT_SAMPLE_CAP * 3) {
            n.push_sample(100 + i as u16);
        }
        assert_eq!(n.rtt_samples.len(), RTT_SAMPLE_CAP);
    }
}

#[cfg(test)]
mod empirical_timeout_tests {
    use super::*;

    /// 核心承诺：样本不足时用固定倍数（保守），样本充分后切换到经验分位数
    #[test]
    fn small_samples_use_fixed_slack_large_use_empirical() {
        let a = AdaptiveTimeout::default();
        let mut n = NodeProbe::new("x", 0);
        n.rtt_ms = Some(800);

        // 样本不足 12：走固定倍数（3×800=2400）
        let few: Vec<u16> = vec![800, 810, 790, 805, 795];
        let t_few = a.timeout_ms_for_with(Some(&n), Some(&few));
        assert_eq!(t_few, 2400, "样本不足时应走固定倍数");

        // 样本 ≥12 且 p95 远低于 3×median：应收紧。
        // 注意下限：冷启动时 anchor 回退为节点自身基线，floor = 800×2 = 1600，
        // 会盖过经验值（p95×1.2 ≈ 972）——这是有意的「相对下限」保护，
        // 防止超时塌缩到毫秒级。样本累积 + EWMA 起来后 anchor 才是 fleet 中位数。
        let many: Vec<u16> = vec![
            800, 805, 795, 810, 798, 802, 807, 793, 801, 806, 799, 804, 803, 796, 809, 801,
            797, 805, 800, 804,
        ];
        let t_many = a.timeout_ms_for_with(Some(&n), Some(&many));
        assert!(
            t_many < t_few,
            "样本充分后应改用经验分位数并收紧：{t_many} < {t_few}"
        );
        // 稳态节点的 p95≈810×1.2=972 低于相对下限 1600，故结果等于 floor
        assert_eq!(t_many, 1600, "冷启动下相对下限应生效");
    }

    /// fleet EWMA 起来后（多节点样本累积），anchor 才是全池中位数
    #[test]
    fn empirical_takes_effect_once_fleet_baseline_known() {
        let a = AdaptiveTimeout::default();
        // 先喂一批慢节点，让全局 EWMA 抬高（fleet 中位数变大 → floor 抬高）
        for _ in 0..30 {
            a.observe(2000);
        }
        let mut n = NodeProbe::new("fast", 0);
        n.rtt_ms = Some(300);
        // 样本充分且极稳：经验值远低于 floor，应被 floor 托住（防塌缩）
        let steady: Vec<u16> = vec![300, 305, 295, 302, 298, 303, 297, 301, 299, 304, 296, 300];
        let t = a.timeout_ms_for_with(Some(&n), Some(&steady));
        // fleet≈2000 → floor≈4000，任何经验值都会被托到该量级
        assert!(
            t >= 3500,
            "fleet 慢时应托住快节点的超时，避免误杀，实际 {t}"
        );
    }

    /// 抖动大的节点：经验分位数会自然放宽（自动识别「这个节点就是抖」）
    #[test]
    fn jittery_node_gets_larger_empirical_ceiling() {
        let a = AdaptiveTimeout::default();
        let mut n = NodeProbe::new("x", 0);
        n.rtt_ms = Some(800);
        let steady: Vec<u16> = vec![
            800, 805, 795, 802, 798, 803, 797, 801, 799, 804, 796, 800, 806, 794, 802, 800,
            798, 804, 801, 799,
        ];
        // 同样中位数，但一半是离群高值
        let jittery: Vec<u16> = vec![
            800, 2500, 810, 2400, 805, 2600, 795, 2300, 802, 2550, 798, 2200, 806, 2450,
            799, 2350, 803, 2505, 801, 2400,
        ];
        let t_steady = a.timeout_ms_for_with(Some(&n), Some(&steady));
        let t_jit = a.timeout_ms_for_with(Some(&n), Some(&jittery));
        // 稳态：经验 p95≈810×1.2=972 → 落到相对下限 1600
        // 抖动：经验 p95≈2500×1.2=3000 → 高于下限，取 3000+
        // 关键性质：抖动节点的超时**由经验数据决定**，不靠固定倍数兜底
        assert!(
            t_jit > t_steady,
            "抖动节点应获得大于稳态节点的余量：{t_jit} vs {t_steady}"
        );
        assert!(
            t_jit >= 2500,
            "抖动节点的超时应覆盖其历史 p95（约 2500ms），实际 {t_jit}"
        );
    }

    /// 关键收益：固定 3× 对多数节点过度宽松
    #[test]
    fn empirical_is_tighter_than_fixed_slack_for_typical_node() {
        let a = AdaptiveTimeout::default();
        let mut n = NodeProbe::new("x", 0);
        // 巴西节点实测：中位 894，p95 1371，3×median=2682 过度宽松
        n.rtt_ms = Some(894);
        let samples: Vec<u16> = vec![
            894, 900, 890, 1371, 895, 888, 1360, 892, 1371, 897, 891, 1365, 893, 899, 889,
            1370, 896, 887, 1362, 894,
        ];
        let t = a.timeout_ms_for_with(Some(&n), Some(&samples));
        assert!(
            t < 894 * 3,
            "经验分位数应显著紧于固定 3 倍：{t} vs {}",
            894 * 3
        );
    }

    /// 经验分位数不得把超时压到「绝对下限」以下
    ///
    /// floor 是相对量纲（fleet_median × floor_ratio，另有 500ms 绝对兜底），
    /// 故此处断言的是那个绝对兜底——它是防止超时退化成毫秒级的最后防线。
    #[test]
    fn empirical_respects_absolute_floor() {
        let a = AdaptiveTimeout::default();
        let mut n = NodeProbe::new("x", 0);
        n.rtt_ms = Some(20);
        // 极快且极稳的节点：经验 p95×1.2 仅 24ms，必须被兜底托住
        let fast: Vec<u16> = vec![
            20, 21, 19, 22, 20, 21, 19, 20, 22, 21, 19, 20, 21, 20, 22, 19, 21, 20, 19, 21,
        ];
        let t = a.timeout_ms_for_with(Some(&n), Some(&fast));
        assert!(t >= 500, "极快节点的超时 {t} 不应低于 500ms 绝对兜底");
    }

    /// 样本持久化后跨重启回填（这是「用得越久越准」的前提）
    #[test]
    fn samples_survive_snapshot_roundtrip() {
        let mut n = NodeProbe::new("x", 0);
        for i in 0..20u16 {
            n.record_success(800 + i * 5, 0);
        }
        let rec = crate::core::stats_db::ProbeStateRecord {
            node_tag: n.tag.clone(),
            rtt_ms: n.rtt_ms,
            last_ok_at: n.last_ok_at,
            fail_streak: n.fail_streak,
            rtt_samples: n.rtt_samples.clone(),
        };
        // 模拟落库 → 读回 → 重建节点
        let mut t = ProbeTable::new();
        t.reset_to(&["x".to_string()], 0);
        t.restore_from(&[rec], 0, 120_000);
        assert_eq!(
            t.get("x").unwrap().rtt_samples.len(),
            n.rtt_samples.len(),
            "样本必须跨「持久化」往返保留"
        );
    }

    /// 回归：门槛必须是 20 而非更小
    ///
    /// 经验 p95 在 n<20 时退化为「最大值」（n=12 → 索引 11 = max），
    /// 等于拿单个离群样本当下限——比固定倍数更不稳。20 是 p95 首次取得
    /// 真实分辨率的位置。门槛若被下调，本测试会失败。
    #[test]
    fn empirical_threshold_is_guarded_against_quantile_degeneration() {
        assert_eq!(
            AdaptiveTimeout::MIN_SAMPLES_FOR_EMPIRICAL,
            20,
            "低于 20 时 p95 退化为 max，经验值不可信"
        );
        // 实证：n=19 的 p95 必然是 max，n=20 才不是
        let degenerate = [800u16, 2500, 810, 2400, 805, 2600, 795, 2300, 802, 2550, 798, 2200, 806, 2450, 799, 2350, 803, 2505, 801]
            .to_vec();
        let mut with20 = degenerate.clone();
        with20.push(2400);
        let a = AdaptiveTimeout::default();
        let t19 = a.timeout_ms_for_with(None, Some(&degenerate));
        let t20 = a.timeout_ms_for_with(None, Some(&with20));
        assert!(
            t19 > t20,
            "n=19 应被当作「样本不足」而更保守，{t19} vs {t20}"
        );
    }

    /// 样本有界：长期运行不得让节点态无限膨胀
    #[test]
    fn sample_ring_stays_bounded_after_long_run() {
        let mut n = NodeProbe::new("x", 0);
        for i in 0..500u16 {
            n.record_success(1000 + (i % 300) as u16, 0);
        }
        assert_eq!(n.rtt_samples.len(), RTT_SAMPLE_CAP);
    }
}
