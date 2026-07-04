/**
 * Auroweave 应用常量
 * 作者: TanXiang
 *
 * 规则：端口号、超时、测速参数等魔法值统一在此定义，禁止在业务代码中硬编码
 */

// ============================================================
// Sing-box 配置
// ============================================================

/** 锁定的 sing-box 版本（与 Cargo.toml 保持同步） */
export const SINGBOX_VERSION = "1.25.4";

/** sing-box ClashAPI 监听地址 */
export const SINGBOX_API_HOST = "127.0.0.1";
export const SINGBOX_API_PORT = 9090;
export const SINGBOX_API_BASE = `http://${SINGBOX_API_HOST}:${SINGBOX_API_PORT}`;
export const SINGBOX_WS_BASE = `ws://${SINGBOX_API_HOST}:${SINGBOX_API_PORT}`;

// ============================================================
// WebSocket 主题路径
// ============================================================

export const WS_PATH_TRAFFIC = `${SINGBOX_WS_BASE}/traffic`;
export const WS_PATH_CONNECTIONS = `${SINGBOX_WS_BASE}/connections`;
export const WS_PATH_LOGS = `${SINGBOX_WS_BASE}/logs`;

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
export const DEFAULT_LATENCY_TEST_URL = "https://www.gstatic.com/generate_204";
export const DEFAULT_LATENCY_TEST_INTERVAL_SEC = 300; // 5 分钟
export const DEFAULT_LATENCY_TEST_TOLERANCE_MS = 50;

// ============================================================
// 吞吐量测速参数
// ============================================================

/** 单节点测速时长上限（秒） */
export const THROUGHPUT_TEST_DURATION_SEC = 8;

/** 测速数据包默认大小（字节，用于估算流量消耗提示） */
export const THROUGHPUT_TEST_CHUNK_BYTES = 10 * 1024 * 1024; // 10 MB

// ============================================================
// 超时配置（ms）
// ============================================================

export const IPC_TIMEOUT_MS = 10_000;        // 普通 IPC 命令超时
export const SUBSCRIPTION_FETCH_TIMEOUT_MS = 30_000; // 订阅拉取超时
export const LATENCY_TEST_TIMEOUT_MS = 5_000; // 单次延迟测试超时

// ============================================================
// WebSocket 重连策略
// ============================================================

export const WS_RECONNECT_DELAY_MS = 1_000;   // 初始重连间隔
export const WS_RECONNECT_MAX_DELAY_MS = 30_000;
export const WS_RECONNECT_MAX_RETRIES = 10;

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

export const WINDOW_DEFAULT_WIDTH = 960;
export const WINDOW_DEFAULT_HEIGHT = 640;
export const WINDOW_MIN_WIDTH = 960;
export const WINDOW_MIN_HEIGHT = 600;

// ============================================================
// UI 交互
// ============================================================

/** 设置图标与关闭按钮之间的最小间距（px），防误触 */
export const SETTINGS_CLOSE_MIN_GAP_PX = 60;

/** Dashboard 启动卡片数量 */
export const DASHBOARD_CARD_COUNT = 3;

/** 最近使用策略组胶囊最大数量 */
export const RECENT_GROUPS_MAX = 4;
