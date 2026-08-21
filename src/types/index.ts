/**
 * Auroweave 全局 TypeScript 类型定义
 * 作者: TanXiang
 */

// ============================================================
// 通用 API 响应结构
// ============================================================

export interface ApiResponse<T = unknown> {
  success: boolean;
  data?: T;
  error?: string;
  code?: number;
}

// ============================================================
// 代理节点与分组
// ============================================================

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

export interface LatencyResult {
  latency: number;
  tested_at: number;
}

export interface ThroughputResult {
  download_bps: number;
  upload_bps: number;
  tested_at: number;
}

export interface ProxyNode {
  tag: string;
  type: ProxyProtocol;
  region?: string;
  country_code?: string;
  latency?: LatencyResult;
  throughput?: ThroughputResult;
  is_active?: boolean;
}

export interface ProxyGroup {
  tag: string;
  type: Extract<ProxyProtocol, "selector" | "urltest" | "loadbalance">;
  proxies: string[];
  now?: string;
  url?: string;
  interval?: number;
}

export interface Connection {
  id: string;
  process?: string;
  processPath?: string;
  destination: string;
  destinationIP?: string;
  port: number;
  network?: "tcp" | "udp" | string;
  type?: string;
  outbound: string;
  chains?: string[];
  rule: string;
  rulePayload?: string;
  upload_bytes: number;
  download_bytes: number;
  upload_speed?: number;
  download_speed?: number;
  start: number;
  dnsMode?: string;
}

export interface TrafficSnapshot {
  download_speed: number;
  upload_speed: number;
  total_download: number;
  total_upload: number;
  active_connections: number;
}

export interface ConnectionStats {
  today_proxied: number;
  today_direct: number;
  today_blocked: number;
  date: string;
}

export interface DnsAuditRecord {
  id: string;
  domain: string;
  rule_description: string;
  outbound_type: "proxy" | "direct" | "block";
  outbound_tag: string;
  resolved_ips: string[];
  semantic_text?: string;
  semantic_icon?: string;
  timestamp: number;
}

export type SubscriptionFormat = "clash" | "mihomo" | "v2ray" | "singbox";

export interface Subscription {
  id: string;
  name: string;
  url: string;
  format: SubscriptionFormat;
  last_updated?: number;
  node_count?: number;
  is_active?: boolean;
  traffic?: {
    upload: number;
    download: number;
    total: number;
    expire?: number;
  };
}

// ============================================================
// 节点排序与自定义分组
// ============================================================

export type NodeSortKey = "default" | "name" | "latency" | "protocol";
export type SortOrder = "asc" | "desc";

export interface NodeSortConfig {
  key: NodeSortKey;
  order: SortOrder;
}

/** 自定义分组匹配规则类型 */
export type GroupMatchType = "keyword" | "regex" | "protocol";

/** 自定义分组规则定义 */
export interface CustomGroupRule {
  id: string;
  name: string;
  enabled: boolean;
  match_type: GroupMatchType;
  /** 关键词列表 (match_type=keyword 时生效) */
  keywords: string[];
  /** 正则表达式 (match_type=regex 时生效) */
  pattern: string;
  /** 协议类型 (match_type=protocol 时生效, 如 vmess, trojan) */
  protocols: string[];
  /** 排序优先级，数字越小越靠前 */
  order: number;
}

export type SpeedTestStatus = "idle" | "testing_latency" | "testing_speed" | "done" | "error";

export interface SpeedTestTask {
  node_tag: string;
  status: SpeedTestStatus;
  progress?: number;
  result?: ThroughputResult & LatencyResult;
  error?: string;
}

export interface SystemProcess {
  name: string;
  exe_path?: string;
  pids: number[];
  bound_outbound?: string;
  is_uwp?: boolean;
  uwp_package_name?: string;
  icon_base64?: string;
}


export type AutomationTriggerType = "ssid" | "network_type" | "time";
export type AutomationActionType = "switch_proxy_mode" | "switch_node" | "switch_group";

export interface AutomationRule {
  id: string;
  name: string;
  enabled: boolean;
  trigger: {
    type: AutomationTriggerType;
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
  auto_start: boolean;
  tun_enabled: boolean;
  /** TUN 虚拟网卡名称，显示在 Windows 网络适配器列表中，默认 "Auroweave" */
  tun_interface_name: string;
  topology_enabled: boolean;
  performance_mode: boolean;
  command_palette_hotkey: string;
  speed_test_urls: string[];
  auto_group_on_import: boolean;

  // 网络代理端口与测速超时可配置项
  mixed_port: number;
  clash_api_port: number;
  speed_test_url: string;
  speed_test_timeout_secs: number;
  connection_timeout_secs: number;
  enable_app_traffic_tracking: boolean;

  minimize_to_tray?: boolean;
  start_minimized?: boolean;
  hide_dock_on_close?: boolean;
  show_tray_speed?: boolean;
  allow_lan?: boolean;


  // 延迟测试配置
  latency_test_concurrency?: number;
  latency_test_timeout_ms?: number;
  latency_test_url?: string;
  core: {
    runMode: string;
    service: {
      installedVersion: string | null;
      lastKnownStatus: string;
      lastFallbackReason: string | null;
    };
  };
}


export interface SingboxUpdateInfo {
  current_version: string;
  latest_version: string;
  has_update: boolean;
  release_notes: string;
  published_at: string;
  download_url?: string;
  download_size: number;
}
