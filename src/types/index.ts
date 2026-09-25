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

// ============================================================
// AI 服务解锁检测（Gemini / Claude / ChatGPT）
// ============================================================

/** 检测目标服务 */
export type UnlockServiceId = "gemini" | "claude" | "chatgpt";

/** 单服务解锁状态（与后端 core/unlock_check.rs UnlockStatus 对齐） */
export type UnlockStatus = "yes" | "no" | "risky" | "failed";

/** 单节点解锁检测结果（services 为服务名→状态映射） */
export interface UnlockCheckResult {
  node_tag: string;
  services: Partial<Record<UnlockServiceId, UnlockStatus>>;
  egress_ip?: string;
  country_code?: string;
  hosting?: boolean;
  proxy_flag?: boolean;
  isp?: string;
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

export interface SubscriptionUserInfo {
  upload_bytes?: number;
  download_bytes?: number;
  total_bytes?: number;
  expire_timestamp?: number;
}

export interface SubscriptionFilterRule {
  include_pattern?: string;
  exclude_pattern?: string;
  rename_pattern?: string;
  rename_replace?: string;
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
  format: SubscriptionFormat | string;
  source_type?: "remote" | "local_file" | "clipboard" | string;
  local_file_path?: string;
  user_agent?: string;
  auto_update_interval_hours?: number;
  last_updated?: number;
  node_count?: number;
  is_active?: boolean;
  user_info?: SubscriptionUserInfo;
  filter_rule?: SubscriptionFilterRule;
  traffic?: {
    upload: number;
    download: number;
    total: number;
    expire?: number;
  };
}

export interface ParsedOutboundNode {
  tag: string;
  type: string;
  server?: string;
  server_port?: number;
  raw_json: Record<string, any>;
}

export interface SubscriptionInspectData {
  id: string;
  name: string;
  format: string;
  node_count: number;
  /** 清洗前原始文本（Base64 订阅已解码为明文 URI 列表） */
  raw_content: string;
  /** 解码前的原始缓存文本（仅当 raw_content 经 Base64 解码时提供） */
  raw_content_original?: string | null;
  parsed_nodes: ParsedOutboundNode[];
  final_config_json: string;
}


// ============================================================
// 节点排序与自定义分组
// ============================================================

export type NodeSortKey = "default" | "name" | "latency" | "protocol" | "unlock";
export type SortOrder = "asc" | "desc";

export interface NodeSortConfig {
  key: NodeSortKey;
  order: SortOrder;
}

/** 自定义分组匹配规则类型（unlock = 按解锁检测结果匹配） */
export type GroupMatchType = "keyword" | "regex" | "protocol" | "unlock";

/** 自定义分组类型：virtual=仅前端本地匹配展示；其余生成内核真实策略组 */
export type CustomGroupType = "virtual" | "selector" | "urltest" | "balance";

/** 解锁匹配规则的字段（match_type=unlock 时生效） */
export interface UnlockMatchConfig {
  /** 目标服务 */
  service: UnlockServiceId;
  /** 期望状态（yes=可用 / no=封锁 / risky=风控疑似 / failed=不可达） */
  status: UnlockStatus;
}

/** 自定义分组规则定义 */
export interface CustomGroupRule {
  id: string;
  name: string;
  enabled: boolean;
  match_type: GroupMatchType;
  /** 分组类型：默认 virtual（不进内核）；selector/urltest/balance 生成真实策略组 */
  group_type?: CustomGroupType;
  /** 关键词列表 (match_type=keyword 时生效) */
  keywords: string[];
  /** 正则表达式 (match_type=regex 时生效) */
  pattern: string;
  /** 协议类型 (match_type=protocol 时生效, 如 vmess, trojan) */
  protocols: string[];
  /** 解锁匹配配置 (match_type=unlock 时生效) */
  unlock?: UnlockMatchConfig;
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

  // DNS 配置（sing-box 1.14.0）
  /** DNS 解析模式："fakeip"（默认推荐）/ "realip"（真实 IP 解析） */
  dns_mode?: "fakeip" | "realip";
  /** 远端 DoH 服务器地址（type: https 的 server 字段） */
  dns_remote_doh?: string;
  /** 节点域名解析专用直连 DoH（bootstrap）；IP 或 DoH 域名，默认 223.5.5.5 */
  dns_bootstrap_doh?: string;
  /** bootstrap 备用直连 DoH（异构运营商对冲负缓存毒化）；IP 或 DoH 域名，默认 1.12.12.12 */
  dns_bootstrap_backup_doh?: string;
  /** 自定义分组规则（CustomGroupRule JSON 数组；真实组类型由后端生成内核策略组） */
  custom_group_rules?: any[];
  /** DNS 查询超时秒数（内核 dns.timeout） */
  dns_timeout_secs?: number;
  /** 乐观 DNS 缓存开关（过期缓存立即返回 + 后台刷新） */
  dns_optimistic_cache?: boolean;
  /** 智能分流 v2：按本地解析结果是否国内 IP 判定直连（evaluate/match_response/respond） */
  dns_smart_routing_v2?: boolean;
  // TUN 进阶（sing-box 1.14.0）
  /** TUN DNS 模式：hijack（默认劫持接口 DNS）/ disabled（不接管系统 DNS） */
  tun_dns_mode?: string;
  /** UDP NAT 会话上限，0 = 内核按内存自适应 */
  udp_nat_max?: number;

  minimize_to_tray?: boolean;
  start_minimized?: boolean;
  hide_dock_on_close?: boolean;
  show_tray_speed?: boolean;
  allow_lan?: boolean;


  // 延迟测试配置
  latency_test_concurrency?: number;
  latency_test_timeout_ms?: number;
  latency_test_url?: string;
  /** 手动延迟测试：预热持久连接后统计第二次请求 RTT（类似 Mihomo unified-delay） */
  latency_unified_delay?: boolean;
  /** 统一延迟是否复用预热连接；关闭后第二次探测使用冷连接，仅用于对照诊断 */
  latency_persistent_reuse?: boolean;
  /** 置顶收藏的节点 tag 列表（排序时恒排最前） */
  pinned_nodes?: string[];
  /** 首页右翼统计胶囊显隐开关（设置-首页显示；缺省视为开启） */
  dashboard_show_connections?: boolean;
  dashboard_show_current_node?: boolean;
  dashboard_show_egress_ip?: boolean;
  dashboard_show_total_traffic?: boolean;

  // 解锁检测判据（空串 = 内置默认；AI 服务页面混淆 ID 轮换后可在此更新）
  unlock_gemini_marker?: string;
  unlock_claude_block_marker?: string;
  unlock_chatgpt_block_marker?: string;

  // 测试内核实例（plan-N）
  /** test-core 端口基址（0 = 默认 40040；冲突自动 +1000 偏移） */
  test_core_port_base?: number;
  /** test-core 批量探测节点级并发上限（2-16，默认 8） */
  unlock_test_concurrency?: number;
  /** 上下行并行测速（默认关=串行保精度） */
  speedtest_parallel_updown?: boolean;
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
