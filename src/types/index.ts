/**
 * Auroweave 全局 TypeScript 类型定义
 * 作者: TanXiang
 *
 * 规则：
 * - 所有跨组件共享的类型在此定义
 * - 字段命名与 Rust 端 serde struct 保持一致（snake_case）
 * - 禁止使用 any
 */

// ============================================================
// 通用 API 响应结构（所有 IPC 命令统一返回此结构）
// ============================================================

export interface ApiResponse<T = unknown> {
  success: boolean;
  data?: T;
  error?: string;
  /** 错误码，便于前端做分类处理 */
  code?: number;
}

// ============================================================
// 代理节点与分组
// ============================================================

/** 节点协议类型 */
export type ProxyProtocol =
  | "vmess"
  | "vless"
  | "trojan"
  | "shadowsocks"
  | "hysteria2"
  | "tuic"
  | "wireguard"
  | "direct"
  | "block"
  | "dns"
  | "selector"
  | "urltest"
  | "loadbalance";

/** 延迟测速结果 */
export interface LatencyResult {
  /** 延迟 ms，-1 表示超时/不可达 */
  latency: number;
  tested_at: number; // Unix timestamp (ms)
}

/** 吞吐量测速结果 */
export interface ThroughputResult {
  /** 下载速度，单位 bytes/s */
  download_bps: number;
  /** 上传速度，单位 bytes/s */
  upload_bps: number;
  tested_at: number;
}

/** 单个代理节点 */
export interface ProxyNode {
  /** 在 sing-box config 中的唯一标签名 */
  tag: string;
  type: ProxyProtocol;
  /** 地区归属（来自节点名称解析或 GeoIP） */
  region?: string;
  /** 国家/地区代码，用于显示国旗 emoji */
  country_code?: string;
  /** 延迟测速结果（最近一次） */
  latency?: LatencyResult;
  /** 吞吐量测速结果（手动触发后填充） */
  throughput?: ThroughputResult;
  /** 是否为当前选中节点（selector 分组中） */
  is_active?: boolean;
}

/** 代理分组 */
export interface ProxyGroup {
  tag: string;
  type: Extract<ProxyProtocol, "selector" | "urltest" | "loadbalance">;
  /** 分组成员（可能是节点 tag 或子分组 tag） */
  proxies: string[];
  /** 当前生效的出站 tag */
  now?: string;
  /** urltest 分组的测速 URL */
  url?: string;
  /** urltest 分组的测速间隔（秒） */
  interval?: number;
}

// ============================================================
// 实时连接与流量
// ============================================================

/** 单个活动连接 */
export interface Connection {
  id: string;
  /** 目标地址 */
  destination: string;
  /** 目标端口 */
  port: number;
  /** 出站节点 tag */
  outbound: string;
  /** 匹配的规则描述 */
  rule: string;
  upload_bytes: number;
  download_bytes: number;
  start: number; // Unix timestamp (ms)
}

/** 实时流量快照（WebSocket 推送） */
export interface TrafficSnapshot {
  /** 当前下载速度（bytes/s，已 EMA 平滑） */
  download_speed: number;
  /** 当前上传速度（bytes/s，已 EMA 平滑） */
  upload_speed: number;
  /** 累计下载 bytes */
  total_download: number;
  /** 累计上传 bytes */
  total_upload: number;
  /** 当前活动连接数 */
  active_connections: number;
}

// ============================================================
// 连接计数统计（Audit 页真实数据支撑）
// ============================================================

export interface ConnectionStats {
  /** 今日代理连接总次数（经过代理出站的连接） */
  today_proxied: number;
  /** 今日直连次数 */
  today_direct: number;
  /** 今日拦截次数 */
  today_blocked: number;
  /** 统计日期（YYYY-MM-DD） */
  date: string;
}

// ============================================================
// DNS 审计记录
// ============================================================

/** DNS 解析事件（Audit 看板数据源） */
export interface DnsAuditRecord {
  id: string;
  /** 查询的域名 */
  domain: string;
  /** 匹配的路由规则描述 */
  rule_description: string;
  /** 出站类型 */
  outbound_type: "proxy" | "direct" | "block";
  /** 出站节点 tag */
  outbound_tag: string;
  /** 解析得到的 IP 列表 */
  resolved_ips: string[];
  /** 语义化翻译文案 */
  semantic_text?: string;
  /** 语义化图标 emoji */
  semantic_icon?: string;
  timestamp: number;
}

// ============================================================
// 订阅
// ============================================================

export type SubscriptionFormat = "clash" | "mihomo" | "v2ray" | "singbox";

export interface Subscription {
  id: string;
  /** 用户自定义名称 */
  name: string;
  url: string;
  format: SubscriptionFormat;
  /** 上次更新时间 */
  last_updated?: number;
  /** 节点总数 */
  node_count?: number;
  /** 流量信息（如订阅头提供） */
  traffic?: {
    upload: number;
    download: number;
    total: number;
    expire?: number;
  };
}

// ============================================================
// 测速
// ============================================================

/** 单节点测速任务状态 */
export type SpeedTestStatus = "idle" | "testing_latency" | "testing_speed" | "done" | "error";

export interface SpeedTestTask {
  node_tag: string;
  status: SpeedTestStatus;
  /** 测速进度 0-100 */
  progress?: number;
  result?: ThroughputResult & LatencyResult;
  error?: string;
}

// ============================================================
// 应用进程（App-Matrix）
// ============================================================

export interface SystemProcess {
  /** 进程名 */
  name: string;
  /** 可执行文件路径 */
  exe_path?: string;
  /** PID 列表（同名进程可能有多个） */
  pids: number[];
  /** 当前绑定的出站 tag（undefined = 跟随默认） */
  bound_outbound?: string;
  /** 是否为 Windows UWP 应用 */
  is_uwp?: boolean;
  /** UWP 包名（Windows 专用） */
  uwp_package_name?: string;
}

// ============================================================
// 场景自动化
// ============================================================

export type AutomationTriggerType = "ssid" | "network_type" | "time";
export type AutomationActionType = "switch_proxy_mode" | "switch_node" | "switch_group";

export interface AutomationRule {
  id: string;
  name: string;
  enabled: boolean;
  trigger: {
    type: AutomationTriggerType;
    /** SSID 名称（trigger = ssid 时使用） */
    ssid?: string;
  };
  action: {
    type: AutomationActionType;
    target?: string;
  };
}

// ============================================================
// 全局设置
// ============================================================

export type ThemeMode = "dark" | "light" | "system";
export type ProxyMode = "global" | "rule" | "direct";
export type AppLanguage = "zh-CN" | "en-US";

export interface AppSettings {
  theme: ThemeMode;
  language: AppLanguage;
  proxy_mode: ProxyMode;
  /** 是否开机自启 */
  auto_start: boolean;
  /** 是否启用 TUN 模式 */
  tun_enabled: boolean;
  /** 是否启用拓扑画布功能（高级） */
  topology_enabled: boolean;
  /** 是否开启性能模式（关闭所有装饰性动效） */
  performance_mode: boolean;
  /** 全局热键（呼出命令框） */
  command_palette_hotkey: string;
  /** 测速服务器 URL 列表 */
  speed_test_urls: string[];
  /** 订阅导入是否自动整理分组 */
  auto_group_on_import: boolean;
}
