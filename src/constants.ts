/**
 * Auroweave 应用常量
 * 作者: TanXiang
 *
 * 规则：端口号、超时、测速参数等魔法值统一在此定义，禁止在业务代码中硬编码
 */

// ============================================================
// Sing-box 配置
// ============================================================

/** sing-box ClashAPI 监听地址（WebSocket 动态 URL 拼接基准） */
export const SINGBOX_API_HOST = "127.0.0.1";
export const SINGBOX_API_PORT = 9090;

// ============================================================
// 测速服务器（默认列表，可在设置页覆盖）
// ============================================================

/** 默认测速 URL 列表，按优先级排序 */
export const DEFAULT_SPEED_TEST_URLS: string[] = [
  "https://speed.cloudflare.com/__down?bytes=10000000",
  "https://fast.com",
  "https://cachefly.cachefly.net/10mb.test",
];

/** 延迟测速 URL（urltest 出站使用） */
export const DEFAULT_LATENCY_TEST_URL = "http://www.gstatic.com/generate_204";
export const DEFAULT_LATENCY_TEST_INTERVAL_SEC = 180; // 3 分钟 (180 秒)
export const DEFAULT_LATENCY_TEST_TOLERANCE_MS = 50;


// ============================================================
// 吞吐量测速参数
// ============================================================

/**
 * 吞吐测速时长事实源（秒）——与 Rust 侧 speedtest/scheduler.rs、commands/speedtest.rs 对齐：
 * 批量测速每节点下载 3s + 上传 3s = 6s；单节点测速下载 5s + 上传 5s = 10s。
 * 历史单常量 8s 与两端实现均不符（预估显示偏差，已修正）。
 */
export const THROUGHPUT_BATCH_DURATION_PER_NODE_SEC = 6;
export const THROUGHPUT_SINGLE_DURATION_SEC = 10;

/** 兼容别名（旧调用方引用）：批量预估口径 */
export const THROUGHPUT_TEST_DURATION_SEC = THROUGHPUT_BATCH_DURATION_PER_NODE_SEC;

/** 测速数据包默认大小（字节，用于估算流量消耗提示） */
export const THROUGHPUT_TEST_CHUNK_BYTES = 10 * 1024 * 1024; // 10 MB

// ============================================================
// 超时配置（ms）
// ============================================================

export const IPC_TIMEOUT_MS = 10_000;        // 普通 IPC 命令超时
export const SUBSCRIPTION_FETCH_TIMEOUT_MS = 30_000; // 订阅拉取超时
export const LATENCY_TEST_TIMEOUT_MS = 5_000; // 单次延迟测试超时

// ============================================================
// 累计流量持久化
// ============================================================

/** 累计流量写 localStorage 的节流间隔（traffic 事件约每秒一条） */
export const TRAFFIC_PERSIST_INTERVAL_MS = 30_000;

// ============================================================
// WebSocket 重连策略
// ============================================================

export const WS_RECONNECT_DELAY_MS = 1_000;   // 初始重连间隔
export const WS_RECONNECT_MAX_DELAY_MS = 10_000;
export const WS_MAX_CONSECUTIVE_FAILURES = 15;

// ============================================================
// 能量核动画
// ============================================================

/** EMA 平滑系数（越小越平滑，越大越灵敏） */
export const ENERGY_EMA_ALPHA = 0.15;

/** 最大参考速度（bytes/s），用于映射动画频率（100 MB/s 为满速） */
export const ENERGY_MAX_SPEED_BPS = 100 * 1024 * 1024;

// ============================================================
// 窗口
// ============================================================

export const WINDOW_DEFAULT_WIDTH = 1020;
export const WINDOW_DEFAULT_HEIGHT = 680;
export const WINDOW_MIN_WIDTH = 1020;
export const WINDOW_MIN_HEIGHT = 640;

// ============================================================
// UI 交互
// ============================================================

/** 设置图标与关闭按钮之间的最小间距（px），防误触 */
export const SETTINGS_CLOSE_MIN_GAP_PX = 60;

/** Dashboard 启动卡片数量 */
export const DASHBOARD_CARD_COUNT = 3;

/** 最近使用策略组胶囊最大数量 */
export const RECENT_GROUPS_MAX = 4;
