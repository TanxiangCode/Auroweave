/// 探测调度 —— 节点延迟的唯一真相源
/// 作者: TanXiang
///
/// # 为什么存在
///
/// 旧架构把「探测一个节点」建模成「组的行为」：每个 urltest 组按自己的 interval
/// 无条件重测全部成员。组之间成员重叠时，同一节点被重复测量 N 次——实测 370 节点
/// 订阅下 8 个 urltest 组每轮发出 805 次探测（唯一节点仅 370，冗余 54%），且全部
/// `interval: 180s` 相等 → 每 180s 一次探测脉冲。
///
/// 脉冲的代价不是带宽而是**排队**：探测与用户流量共用主实例的出站连接资源，
/// 805 次探测挤在瞬间完成时，用户真实请求排在后面，实测 singbox.log 出现 338 次
/// `outbound/urltest[auto]: failed to create session: context deadline exceeded`，
/// 每次 15.2s（github.com / bing.com / googleusercontent 等真实域名）。
///
/// # 本模块的解法
///
/// 1. **测量面与数据面分离**：探测全部走常驻 test-core（独立进程、独立端口段
///    40040+），主实例只服务用户流量。实测 64 并发探测期间主实例 mixed 端口
///    响应 112ms。
/// 2. **单次测量，多处消费**：节点表是唯一真相，组只是视图。
/// 3. **探测频率与决策价值成正比**：分层间隔 + 指数退避 + 哈希抖动。
///
/// # 分层间隔
///
/// | 层 | 定义 | 间隔 |
/// |---|---|---|
/// | Active | 当前选中 / pinned / top-N | 60s |
/// | Standby | 健康备用 | 5min |
/// | Cooling | 连续失败 | 30s→5m→30m→1h |
///
/// 370 节点全部按 180s 探测 = 每小时 7400 次测量，来服务「用户每小时 1 次
/// 选点」——200 倍过采样。分层后稳态约 60 次/分，且从脉冲变为细流。
pub mod table;
pub mod scheduler;
pub mod selector;
pub mod guard;

pub use guard::{ActiveHealthGuard, GuardOutcome, HealthGuardConfig};
pub use scheduler::ProbeScheduler;
pub use selector::{AutoSelector, Decision, SelectorConfig};
pub use table::{AdaptiveTimeout, FailureClass, NodeProbe, ProbeTable, ProbeTier};

/// 探测结果事件名（前端实时刷新延迟列表）
pub const PROBE_PROGRESS_EVENT: &str = "probe-progress";
