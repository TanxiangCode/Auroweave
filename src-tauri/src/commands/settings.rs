/// IPC 命令 — 应用设置
/// 作者: TanXiang
use crate::error::ApiResponse;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoreServiceSettings {
    #[serde(rename = "installedVersion")]
    pub installed_version: Option<String>,
    #[serde(rename = "lastKnownStatus")]
    pub last_known_status: String,
    #[serde(rename = "lastFallbackReason")]
    pub last_fallback_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoreSettings {
    #[serde(rename = "runMode")]
    pub run_mode: String,
    pub service: CoreServiceSettings,
}

fn default_core_settings() -> CoreSettings {
    CoreSettings {
        run_mode: "local".to_string(),
        service: CoreServiceSettings {
            installed_version: None,
            last_known_status: "not_installed".to_string(),
            last_fallback_reason: None,
        },
    }
}

fn default_latency_test_concurrency() -> u32 {
    20
}
fn default_latency_test_timeout_ms() -> u64 {
    5000
}
fn default_latency_test_url() -> String {
    "http://www.gstatic.com/generate_204".to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

fn default_dns_remote_doh() -> String {
    "8.8.8.8".to_string()
}
fn default_dns_bootstrap_doh() -> String {
    "223.5.5.5".to_string()
}
fn default_dns_bootstrap_backup_doh() -> String {
    "1.12.12.12".to_string()
}
fn default_dns_timeout_secs() -> u64 {
    5
}
fn default_dns_mode() -> String {
    "fakeip".to_string()
}
fn default_tun_dns_mode() -> String {
    "hijack".to_string()
}
fn default_udp_nat_max() -> u64 {
    0
}
fn default_unlock_test_concurrency() -> u32 {
    8
}

fn apply_settings_migrations(settings: &mut AppSettings) {
    let legacy_primary = "https://speed.cloudflare.com/__down?bytes=25000000";
    let legacy_fallback = "https://fast.com";
    if settings.speed_test_url == legacy_primary
        && settings.speed_test_urls.len() == 2
        && settings.speed_test_urls[0] == legacy_primary
        && settings.speed_test_urls[1] == legacy_fallback
    {
        settings.speed_test_url = "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-aarch64-apple-darwin.tar.gz".to_string();
        settings.speed_test_urls = vec![
            "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-aarch64-apple-darwin.tar.gz".to_string(),
            "https://ash-speed.hetzner.com/100MB.bin".to_string(),
            "https://proof.ovh.net/files/10Mb.dat".to_string(),
            legacy_primary.to_string(),
        ];
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub theme: String,
    pub language: String,
    pub proxy_mode: String,
    pub auto_start: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default = "default_false")]
    pub start_minimized: bool,
    #[serde(default = "default_false")]
    pub hide_dock_on_close: bool,
    #[serde(default = "default_true")]
    pub show_tray_speed: bool,
    #[serde(default = "default_false")]
    pub allow_lan: bool,
    pub tun_enabled: bool,
    /// TUN 虚拟网卡名称，显示 in Windows 网络适配器列表中，默认 Auroweave
    pub tun_interface_name: String,
    pub topology_enabled: bool,
    pub performance_mode: bool,
    pub command_palette_hotkey: String,
    pub speed_test_urls: Vec<String>,
    pub auto_group_on_import: bool,

    // 网络代理端口与测速超时可配置项
    pub mixed_port: u16,
    pub clash_api_port: u16,
    pub speed_test_url: String,
    pub speed_test_timeout_secs: u64,
    pub connection_timeout_secs: u64,
    pub enable_app_traffic_tracking: bool,

    // DNS 配置（sing-box 1.14.0：optimistic 缓存 / timeout 快速失败 / 远端 DoH 服务器）
    #[serde(default = "default_dns_remote_doh")]
    pub dns_remote_doh: String,
    /// 节点域名解析专用直连 DoH（bootstrap）：不经代理防回环、不经运营商递归防污染
    #[serde(default = "default_dns_bootstrap_doh")]
    pub dns_bootstrap_doh: String,
    /// bootstrap 的备用直连 DoH（异构运营商，默认 dnspod 1.12.12.12）：
    /// 主解析器负缓存毒化（机场子域轮换删除窗口被 SOA 600s 负缓存放大）时，
    /// 由 response_rcode=NXDOMAIN/SERVFAIL 规则切到备用解析器对冲
    #[serde(default = "default_dns_bootstrap_backup_doh")]
    pub dns_bootstrap_backup_doh: String,
    #[serde(default = "default_dns_timeout_secs")]
    pub dns_timeout_secs: u64,
    #[serde(default = "default_true")]
    pub dns_optimistic_cache: bool,
    /// 智能分流 v2（1.14.0 evaluate/match_response/respond 响应级分流）：
    /// 开启后 CN 判定从 geosite 域名名单升级为"本地解析结果是否国内 IP"，
    /// 默认关闭（opt-in 灰度）；需 geoip-cn rule-set 就绪，未就绪时生成器不注入
    #[serde(default = "default_false")]
    pub dns_smart_routing_v2: bool,
    /// DNS 解析模式："fakeip" (默认，推荐) 或 "realip"
    #[serde(default = "default_dns_mode")]
    pub dns_mode: String,

    /// 自定义分组规则（前端「自定义区域管理」维护的 CustomGroupRule JSON 数组）：
    /// group_type=virtual 仅前端本地匹配展示；selector/urltest/balance 由
    /// ConfigBuilder 生成内核真实策略组（tag = custom-{name}）
    #[serde(default)]
    pub custom_group_rules: Vec<serde_json::Value>,

    // TUN 进阶选项（sing-box 1.14.0：dns_mode 显式化 + UDP NAT 上限）
    #[serde(default = "default_tun_dns_mode")]
    pub tun_dns_mode: String,
    #[serde(default = "default_udp_nat_max")]
    pub udp_nat_max: u64,

    // 延迟测试并发控制与超时参数
    #[serde(default = "default_latency_test_concurrency")]
    pub latency_test_concurrency: u32,
    #[serde(default = "default_latency_test_timeout_ms")]
    pub latency_test_timeout_ms: u64,
    #[serde(default = "default_latency_test_url")]
    pub latency_test_url: String,
    /// 手动延迟测试使用独立 test-core：先预热持久连接，再统计第二次请求 RTT。
    #[serde(default)]
    pub latency_unified_delay: bool,

    #[serde(default = "default_true")]
    pub latency_persistent_reuse: bool,

    /// 置顶收藏的节点 tag 列表（节点卡片星标，排序时恒排最前）
    #[serde(default)]
    pub pinned_nodes: Vec<String>,

    // 解锁检测判据（AI 服务页面混淆 ID / 封锁特征会随版本轮换，settings 可更新；
    // 空串回退内置默认，详见 core/unlock_check.rs）
    #[serde(default)]
    pub unlock_gemini_marker: String,
    #[serde(default)]
    pub unlock_claude_block_marker: String,
    #[serde(default)]
    pub unlock_chatgpt_block_marker: String,

    // 测试内核实例（plan-N）：端口基址与探测并发
    /// test-core 专属端口基址（0 = 内置默认 40040；冲突时自动 +1000 偏移）
    #[serde(default)]
    pub test_core_port_base: u16,
    /// test-core 批量探测的节点级并发上限（2-16，默认 8）
    #[serde(default = "default_unlock_test_concurrency")]
    pub unlock_test_concurrency: u32,

    /// test-core 批量吞吐测速并发（默认 1=串行——并发抢带宽数值失真；
    /// 迁移只为零打扰；上限 4）
    #[serde(default)]
    pub speedtest_test_concurrency: u32,

    /// 上下行并行测速（默认关=串行保精度；开启后单节点耗时约减半，
    /// 上下行同时挤占带宽数值轻微偏低——test-core 专属端口下互不干扰）
    #[serde(default)]
    pub speedtest_parallel_updown: bool,

    /// 首页右翼统计胶囊显隐开关（设置-首页显示；true=显示，缺省视为 true
    /// 兼容旧 settings.json）
    #[serde(default = "default_true")]
    pub dashboard_show_connections: bool,
    #[serde(default = "default_true")]
    pub dashboard_show_current_node: bool,
    #[serde(default = "default_true")]
    pub dashboard_show_egress_ip: bool,
    #[serde(default = "default_true")]
    pub dashboard_show_total_traffic: bool,

    /// 分组测速配置覆盖（group tag -> interval 秒 / tolerance 毫秒 / url），
    /// 由 GroupEditModal 保存，ConfigBuilder 生成 urltest 出站时应用
    #[serde(default)]
    pub group_configs: std::collections::HashMap<String, GroupTestConfig>,

    #[serde(default = "default_core_settings")]
    pub core: CoreSettings,
}

/// 单个分组的测速配置覆盖项（未设置的字段保持内置默认值）
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GroupTestConfig {
    /// 自动测速心跳间隔（秒），前端已把 "3m"/"1h" 归一为秒
    pub interval: Option<u64>,
    /// 测速容差（毫秒）
    pub tolerance: Option<u64>,
    /// 测速目标 URL
    pub url: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "zh-CN".to_string(),
            proxy_mode: "rule".to_string(),
            auto_start: false,
            minimize_to_tray: true,
            start_minimized: false,
            hide_dock_on_close: false,
            show_tray_speed: true,
            allow_lan: false,
            tun_enabled: false,

            tun_interface_name: "Auroweave".to_string(),
            topology_enabled: false,
            performance_mode: false,
            command_palette_hotkey: "CommandOrControl+Space".to_string(),
            speed_test_urls: vec![
                "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-aarch64-apple-darwin.tar.gz".to_string(),
                "https://ash-speed.hetzner.com/100MB.bin".to_string(),
                "https://proof.ovh.net/files/10Mb.dat".to_string(),
                "https://speed.cloudflare.com/__down?bytes=25000000".to_string(),
            ],
            auto_group_on_import: true,

            // 默认端口与超时设定
            mixed_port: 8890,
            clash_api_port: 9090,
            speed_test_url: "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-aarch64-apple-darwin.tar.gz".to_string(),
            speed_test_timeout_secs: 5,
            connection_timeout_secs: 15,
            enable_app_traffic_tracking: true,

            // DNS 默认（sing-box 1.14.0）
            dns_remote_doh: default_dns_remote_doh(),
            dns_bootstrap_doh: default_dns_bootstrap_doh(),
            dns_bootstrap_backup_doh: default_dns_bootstrap_backup_doh(),
            dns_timeout_secs: default_dns_timeout_secs(),
            dns_optimistic_cache: true,
            dns_smart_routing_v2: false,
            dns_mode: default_dns_mode(),
            custom_group_rules: Vec::new(),
            tun_dns_mode: default_tun_dns_mode(),
            udp_nat_max: default_udp_nat_max(),

            // 延迟测试配置
            latency_test_concurrency: 20,
            latency_test_timeout_ms: 5000,
            latency_test_url: "http://www.gstatic.com/generate_204".to_string(),
            latency_unified_delay: false,
            latency_persistent_reuse: true,

            // 置顶收藏节点（默认空）
            pinned_nodes: Vec::new(),

            // 解锁检测判据（默认空串 = 内置默认判据，UnlockCheckParams::from_settings 回退）
            unlock_gemini_marker: String::new(),
            unlock_claude_block_marker: String::new(),
            unlock_chatgpt_block_marker: String::new(),

            // 测试内核实例（plan-N）：0 = 默认基址；并发默认 8
            test_core_port_base: 0,
            unlock_test_concurrency: default_unlock_test_concurrency(),
            speedtest_test_concurrency: 1,
            speedtest_parallel_updown: false,

            // 首页右翼统计胶囊（默认全开，可到设置-首页显示关闭）
            dashboard_show_connections: true,
            dashboard_show_current_node: true,
            dashboard_show_egress_ip: true,
            dashboard_show_total_traffic: true,

            // 分组测速配置覆盖（默认空，全部使用内置默认值）
            group_configs: std::collections::HashMap::new(),

            core: default_core_settings(),
        }
    }
}

fn get_settings_path() -> PathBuf {
    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);
    config_dir.join("settings.json")
}

// settings.json 读取缓存：mtime 未变时复用上次解析结果
//
// 性能：settings_get_internal 全仓 20+ 调用点（3s 状态轮询/守护/托盘/测速等
// 每拍多次），每次读盘+serde 解析；settings.json 仅由本进程原子写（更新时
// mtime 必变），mtime 缓存安全。外部手改文件的场景：mtime 变化即可感知。
lazy_static::lazy_static! {
    static ref SETTINGS_CACHE: std::sync::Mutex<Option<(std::time::SystemTime, AppSettings)>> =
        std::sync::Mutex::new(None);
}

/// 内部加载设置函数 (供各 Rust 模块使用)
pub fn settings_get_internal(_app_handle: &tauri::AppHandle) -> AppSettings {
    let path = get_settings_path();
    let mtime = fs::metadata(&path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let mut cache = SETTINGS_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((cached_mtime, ref cached)) = *cache {
        if cached_mtime == mtime {
            return cached.clone();
        }
    }

    let mut loaded = if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&content) {
                settings
            } else {
                AppSettings::default()
            }
        } else {
            AppSettings::default()
        }
    } else {
        AppSettings::default()
    };

    apply_settings_migrations(&mut loaded);
    *cache = Some((mtime, loaded.clone()));
    loaded
}

/// 检查 config 的 route.rule_set 中是否已注册指定 tag 的规则集
///（rebuild_config_from_settings 的 dns v2 注入门控：未注册时注入引用它的
///  dns 规则会导致内核初始化期拒载）
fn route_rule_set_registered(config: &serde_json::Value, tag: &str) -> bool {
    config
        .get("route")
        .and_then(|r| r.get("rule_set"))
        .and_then(|rs| rs.as_array())
        .map(|arr| {
            arr.iter()
                .any(|rs| rs.get("tag").and_then(|t| t.as_str()) == Some(tag))
        })
        .unwrap_or(false)
}

/// 从 config 的 outbounds 提取节点服务器域名（与 ConfigBuilder::build 阶段3 同语义：
/// 非空且非 IP 字面量的 server 字段，排序去重——dns.rules 整块替换的数据来源）
fn extract_outbound_server_domains(config: &serde_json::Value) -> Vec<String> {
    let mut domains: Vec<String> = config
        .get("outbounds")
        .and_then(|o| o.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|o| o.get("server").and_then(|s| s.as_str()))
                .filter(|s| !s.is_empty() && s.parse::<std::net::IpAddr>().is_err())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    domains.sort();
    domains.dedup();
    domains
}
/// 从 config 的 outbounds 提取真实存在的 tag 集合。
fn config_outbound_tags(config: &serde_json::Value) -> std::collections::HashSet<String> {
    crate::commands::routing::outbound_tags_from_config(config)
}

/// 生成 remote DNS server；无 proxy 时不写 detour。
fn canonical_remote_dns_server(addr: &str, detour_tag: Option<&str>) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "tag": "remote",
        "type": "https",
        "server": if addr.trim().is_empty() { "8.8.8.8" } else { addr.trim() },
    });
    if let Some(tag) = detour_tag {
        obj["detour"] = serde_json::json!(tag);
    }
    obj
}

/// 核心重构函数：将 AppSettings 中的全部可配置项（端口、TUN、路由等）统一同步到 config.json
pub fn rebuild_config_from_settings(
    app_handle: &tauri::AppHandle,
) -> Result<(), crate::error::AppError> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        return Ok(());
    }

    let settings = settings_get_internal(app_handle);

    // 读取当前 config.json
    let content = fs::read_to_string(&config_path)
        .map_err(|e| crate::error::AppError::Io(format!("读取 config.json 失败: {}", e)))?;
    let mut config_val: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| crate::error::AppError::Config(format!("解析 config.json 失败: {}", e)))?;

    let mut modified = false;

    // 0. 顶层 $schema 注入（plan-Q A.4-3，与 ConfigBuilder::build 同步）：
    // 用户手改 config.json 时兼容编辑器（VS Code + JSON Schema 插件）可自动校验
    {
        if let Some(obj) = config_val.as_object_mut() {
            if obj.get("$schema").and_then(|v| v.as_str())
                != Some("https://sing-box.sagernet.org/schema.json")
            {
                obj.insert(
                    "$schema".to_string(),
                    serde_json::json!("https://sing-box.sagernet.org/schema.json"),
                );
                modified = true;
            }
        }
    }

    // 1. 同步 Inbounds (包括端口 mixed_port、allow_lan 监听与开关 tun_enabled)
    if let Some(inbounds) = config_val
        .get_mut("inbounds")
        .and_then(|i| i.as_array_mut())
    {
        inbounds.clear();
        // 混合代理入口
        inbounds.push(serde_json::json!({
            "type": "mixed",
            "tag": "mixed-in",
            "listen": if settings.allow_lan { "0.0.0.0" } else { "127.0.0.1" },
            "listen_port": settings.mixed_port
        }));

        // TUN 模式入口 (如果启用)
        if settings.tun_enabled {
            // macOS 上 TUN 接口名必须以 utun 开头 (如 utun6)，否则 sing-box 报 bad tun name；
            // 但硬编码 utun9 会与其他进程固定抢占同一接口导致冲突。
            // sing-box 支持不指定 interface_name，由系统自动分配空闲 utun 接口，
            // 因此 macOS 下默认置空（除非用户显式填了合法 utun 名称）。
            // Linux 上可使用任意名称 (如 tun0)；Windows 上 interface_name 不生效 (使用 Wintun 驱动)。
            #[cfg(target_os = "macos")]
            let iface_name = {
                let user_name = settings.tun_interface_name.trim();
                if user_name.starts_with("utun")
                    && user_name.len() > 4
                    && user_name[4..].chars().all(|c| c.is_ascii_digit())
                {
                    user_name.to_string()
                } else {
                    // 未显式指定合法 utun 名称时不传 interface_name，让系统自动分配
                    String::new()
                }
            };
            #[cfg(not(target_os = "macos"))]
            let iface_name = if settings.tun_interface_name.trim().is_empty() {
                "Auroweave".to_string()
            } else {
                settings.tun_interface_name.clone()
            };
            // macOS 下 iface_name 为空时省略 interface_name 字段（自动分配）
            let mut tun_inbound = serde_json::json!({
                "type": "tun",
                "tag": "tun-in",
                // 双栈地址（docs/inbound/tun.zh.md 示例同构）：仅有 v4 地址时
                // auto_route 不接管 v6，系统的 IPv6 DNS 解析器（如 2400:3200::1）
                // 会绕过 TUN 直连运营商——被劫持域名（google 等）的毒化答案走 v6
                // 通道抢先返回（v4 劫持链经 DoH 约秒级，毒化答案毫秒级必赢竞争），
                // 表现为"TUN 下谷歌打不开而其它站点正常"。补 v6 地址后 v6 DNS
                // 同进隧道被 hijack-dns 统一劫持。ULA 段避开 mihomo 惯用的
                // fdfe:dcba:9875::/126 与文档示例 9876::/126，防双 TUN 共存撞段
                "address": ["172.19.0.1/30", "fdfe:dcba:9874::1/126"],
                "auto_route": true,
                "strict_route": true,
                "stack": "system",
                // 1.14.0 起默认 dns_mode 即为 hijack（改写系统每接口 DNS 并劫持），
                // 显式写入用户设置值，掌控 TUN 下系统 DNS 行为（hijack/disabled）
                "dns_mode": settings.tun_dns_mode
            });
            // UDP NAT 会话上限（0 = 内核按内存自适应默认 4096-16384；仅 TUN 进阶场景需要收紧）
            if settings.udp_nat_max > 0 {
                if let Some(obj) = tun_inbound.as_object_mut() {
                    obj.insert(
                        "udp_nat_max".to_string(),
                        serde_json::json!(settings.udp_nat_max),
                    );
                }
            }
            if !iface_name.is_empty() {
                if let Some(obj) = tun_inbound.as_object_mut() {
                    obj.insert("interface_name".to_string(), serde_json::json!(iface_name));
                }
            }
            inbounds.push(tun_inbound);
        }
        modified = true;
    }

    // 2. 同步 ClashAPI 端口 + cache_file 路径
    if let Some(experimental) = config_val
        .get_mut("experimental")
        .and_then(|e| e.as_object_mut())
    {
        if let Some(clash_api) = experimental
            .get_mut("clash_api")
            .and_then(|c| c.as_object_mut())
        {
            clash_api.insert(
                "external_controller".to_string(),
                serde_json::json!(format!("127.0.0.1:{}", settings.clash_api_port)),
            );
            modified = true;
        }
        // 确保 cache_file.path 指向数据目录，避免 sing-box 将 cache.db 写入工作目录
        // （开发模式下会触发 Tauri 文件监听导致应用反复重启）
        if let Some(cache_file) = experimental
            .get_mut("cache_file")
            .and_then(|c| c.as_object_mut())
        {
            cache_file.insert(
                "path".to_string(),
                serde_json::json!(crate::get_data_root()
                    .join("cache.db")
                    .to_string_lossy()
                    .to_string()),
            );
            cache_file.insert("store_dns".to_string(), serde_json::json!(true));
            cache_file.insert("store_fakeip".to_string(), serde_json::json!(true));
            modified = true;
        }
    }

    // 2.5 同步分组测速配置覆盖（GroupEditModal 保存的 interval/tolerance/url）
    // 对 config.json 中已存在的 urltest 出站按 tag 应用用户覆盖，
    // 仅覆盖显式设置的字段；ConfigBuilder 全量重建时亦会应用同一配置
    if !settings.group_configs.is_empty() {
        if let Some(outbounds) = config_val
            .get_mut("outbounds")
            .and_then(|o| o.as_array_mut())
        {
            for out in outbounds.iter_mut() {
                let tag = out
                    .get("tag")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                if let Some(cfg) = settings.group_configs.get(&tag) {
                    if out.get("type").and_then(|t| t.as_str()) != Some("urltest") {
                        continue; // 仅 urltest 组支持覆盖
                    }
                    if let Some(obj) = out.as_object_mut() {
                        if let Some(interval) = cfg.interval {
                            obj.insert(
                                "interval".to_string(),
                                serde_json::json!(format!("{}s", interval)),
                            );
                        }
                        if let Some(tolerance) = cfg.tolerance {
                            obj.insert("tolerance".to_string(), serde_json::json!(tolerance));
                        }
                        if let Some(url) = &cfg.url {
                            if !url.trim().is_empty() {
                                obj.insert("url".to_string(), serde_json::json!(url.trim()));
                            }
                        }
                        modified = true;
                    }
                }
            }
        }
    }

    // 当前配置真实存在的 outbound tag 集合。无订阅 minimal 配置只有
    // direct/block，不能继续写入 proxy 引用。
    let valid_outbound_tags = config_outbound_tags(&config_val);
    let has_proxy = valid_outbound_tags.contains("proxy");
    let route_final = if has_proxy {
        Some("proxy".to_string())
    } else if valid_outbound_tags.contains("direct") {
        Some("direct".to_string())
    } else {
        let mut sorted: Vec<String> = valid_outbound_tags.iter().cloned().collect();
        sorted.sort();
        sorted.into_iter().next()
    };
    // 3. 同步代理模式与完整的路由规则（App-Matrix、Custom Rules、Rule-Set）到 route
    // 模式切换统一走 clash_mode 单一真相源：
    //   - route.final 仅引用当前实际存在的 outbound；无 proxy 时回退 direct/实际 tag
    //   - direct 模式由 rules 中前置的 {"clash_mode":"direct","outbound":"direct"} 全量直连
    //   - 运行时 PATCH /configs {"mode":...} 与重启后配置文件行为完全一致
    //   （旧实现热切换只改内存 mode、重启改写 final，两条链路行为不一致且
    //     direct 模式下测速/分流全错）
    if let Some(route) = config_val.get_mut("route").and_then(|r| r.as_object_mut()) {
        if let Some(route_final) = route_final.as_deref() {
            route.insert("final".to_string(), serde_json::json!(route_final));
        } else {
            route.remove("final");
        }

        // 检测 geosite-cn 和 geoip-cn 本地规则集文件是否存在
        let geosite_exists = config_dir.join("geosite-cn.srs").exists();
        let geoip_exists = config_dir.join("geoip-cn.srs").exists();

        // 统一通过 build_full_route_rules 构建并覆盖完整的路由规则
        let full_rules = crate::core::config_builder::build_full_route_rules_filtered(
            geosite_exists,
            geoip_exists,
            Some(&valid_outbound_tags),
        );
        route.insert("rules".to_string(), serde_json::json!(full_rules));

        // 同步 rule_set 注册：geosite 与 geoip 各自独立判断（与 build_full_route_rules
        // 的独立布尔语义对齐；旧实现 geoip 被 geosite 门控，geosite 丢失而 geoip
        // 存在时会引用未注册的 rule-set 导致内核拒载）。
        // M3-1 remote 化与 ConfigBuilder::build 同源：remote + initial_path；
        // 仅在实际存在 proxy 时为 http_client 写入 detour=proxy
        let mk_remote_rule_set = |tag: &str| {
            serde_json::json!({
                "tag": tag,
                "type": "remote",
                "format": "binary",
                "url": if tag == "geosite-cn" {
                    "https://fastly.jsdelivr.net/gh/SagerNet/sing-geosite@rule-set/geosite-cn.srs"
                } else {
                    "https://fastly.jsdelivr.net/gh/SagerNet/sing-geoip@rule-set/geoip-cn.srs"
                },
                "initial_path": config_dir.join(format!("{}.srs", tag)).to_string_lossy().to_string(),
                "http_client": if has_proxy {
                    serde_json::json!({ "detour": "proxy" })
                } else {
                    serde_json::json!({})
                }
            })
        };
        let mut rule_sets = Vec::new();
        if geosite_exists {
            rule_sets.push(mk_remote_rule_set("geosite-cn"));
        }
        if geoip_exists {
            rule_sets.push(mk_remote_rule_set("geoip-cn"));
        }
        if !rule_sets.is_empty() {
            route.insert("rule_set".to_string(), serde_json::json!(rule_sets));
        } else {
            route.remove("rule_set");
        }

        // 同步 experimental.clash_api.default_mode 与 proxy_mode，
        // 保证冷启动（cache 未恢复 mode 时）的 clash_mode 匹配初值与用户设置一致
        if let Some(experimental) = config_val
            .get_mut("experimental")
            .and_then(|e| e.as_object_mut())
        {
            if let Some(clash_api) = experimental
                .get_mut("clash_api")
                .and_then(|c| c.as_object_mut())
            {
                let default_mode = match settings.proxy_mode.as_str() {
                    "global" => "Global",
                    "direct" => "Direct",
                    _ => "Rule",
                };
                clash_api.insert("default_mode".to_string(), serde_json::json!(default_mode));
            }
        }

        modified = true;
    }

    // 4. 移除 log.output 以便统一使用系统日志收集 stdout
    if let Some(log) = config_val.get_mut("log").and_then(|l| l.as_object_mut()) {
        if log.remove("output").is_some() {
            modified = true;
        }
    }

    // 5. 确保 dns 配置规范（远端 DoH 服务器地址 / timeout / optimistic 缓存 + local 校准）
    // dns.rules 整块替换所需的前置数据在可变借用前计算（route.rule_set 注册状态
    // 与 outbound server 域名清单），与 ConfigBuilder::build 共用 build_dns_rules
    let server_domains = extract_outbound_server_domains(&config_val);
    let route_has_geosite = route_rule_set_registered(&config_val, "geosite-cn");
    let route_has_geoip = route_rule_set_registered(&config_val, "geoip-cn");
    if let Some(dns) = config_val.get_mut("dns").and_then(|d| d.as_object_mut()) {
        let is_fake_ip = settings.dns_mode.trim().to_lowercase() != "realip";

        if !dns.contains_key("strategy") {
            dns.insert("strategy".to_string(), serde_json::json!("prefer_ipv4"));
            modified = true;
        }

        // 1.14.0 optimistic 缓存：过期缓存立即返回 + 后台刷新（与 store_dns 持久化配合）
        let optimistic = settings.dns_optimistic_cache;
        if dns.get("optimistic").and_then(|v| v.as_bool()) != Some(optimistic) {
            dns.insert("optimistic".to_string(), serde_json::json!(optimistic));
            modified = true;
        }
        // 1.14.0 DNS 查询超时：上游死亡快速失败，不再拖默认 10s
        let timeout = format!("{}s", settings.dns_timeout_secs.max(1));
        if dns.get("timeout").and_then(|v| v.as_str()) != Some(timeout.as_str()) {
            dns.insert("timeout".to_string(), serde_json::json!(timeout));
            modified = true;
        }

        // 校准 local dns server 为 type: local（附 1.14 neighbor_domain 局域网解析），
        // remote dns server 的 DoH 地址跟随用户设置，
        // bootstrap / bootstrap-backup dns server（节点域名专用直连 DoH 主备）地址跟随设置且缺失时补齐
        let remote_doh = settings.dns_remote_doh.trim().to_string();
        let canonical_bootstrap =
            crate::core::config_builder::canonical_bootstrap_server(&settings.dns_bootstrap_doh);
        let canonical_backup = crate::core::config_builder::canonical_bootstrap_backup_server(
            &settings.dns_bootstrap_backup_doh,
        );
        if let Some(servers) = dns.get_mut("servers").and_then(|s| s.as_array_mut()) {
            for srv in servers.iter_mut() {
                if srv.get("tag").and_then(|t| t.as_str()) == Some("local") {
                    let canonical_local = serde_json::json!({
                        "tag": "local",
                        "type": "local",
                        // 1.14.0：nas / printer.lan 等单标签与局域网后缀走系统邻居解析器
                        "neighbor_domain": [".", ".lan", ".local"]
                    });
                    if *srv != canonical_local {
                        *srv = canonical_local;
                        modified = true;
                    }
                } else if srv.get("tag").and_then(|t| t.as_str()) == Some("remote") {
                    let canonical_remote = canonical_remote_dns_server(
                        &remote_doh,
                        if has_proxy { Some("proxy") } else { None },
                    );
                    if *srv != canonical_remote {
                        *srv = canonical_remote;
                        modified = true;
                    }
                } else if srv.get("tag").and_then(|t| t.as_str()) == Some("bootstrap") {
                    // 校准节点域名专用解析器：地址跟随设置（IP 直用 / 域名附 domain_resolver=local）
                    if *srv != canonical_bootstrap {
                        *srv = canonical_bootstrap.clone();
                        modified = true;
                    }
                } else if srv.get("tag").and_then(|t| t.as_str()) == Some("bootstrap-backup") {
                    // 校准备用解析器（NXDOMAIN/SERVFAIL 对冲链的第二跳）
                    if *srv != canonical_backup {
                        *srv = canonical_backup.clone();
                        modified = true;
                    }
                }
            }
            let has_fakeip = servers
                .iter()
                .any(|s| s.get("tag").and_then(|t| t.as_str()) == Some("fakeip"));
            if is_fake_ip && !has_fakeip {
                servers.push(serde_json::json!({
                    "tag": "fakeip",
                    "type": "fakeip",
                    "inet4_range": "198.18.0.0/15",
                    "inet6_range": "fc00::/18"
                }));
                modified = true;
            } else if !is_fake_ip && has_fakeip {
                servers.retain(|s| s.get("tag").and_then(|t| t.as_str()) != Some("fakeip"));
                modified = true;
            }

            let has_bootstrap = servers
                .iter()
                .any(|s| s.get("tag").and_then(|t| t.as_str()) == Some("bootstrap"));
            if !has_bootstrap {
                servers.push(canonical_bootstrap);
                modified = true;
            }
            let has_backup = servers
                .iter()
                .any(|s| s.get("tag").and_then(|t| t.as_str()) == Some("bootstrap-backup"));
            if !has_backup {
                servers.push(canonical_backup);
                modified = true;
            }
        }

        // sing-box 规范与内核强校验：default server 严禁为 fakeip，final 保持为 remote
        // 境外与代理域名由下方 build_dns_rules 生成的 route -> fakeip 规则承接
        let target_final = "remote";
        if dns.get("final").and_then(|f| f.as_str()) != Some(target_final) {
            dns.insert("final".to_string(), serde_json::json!(target_final));
            modified = true;
        }

        // dns.rules 整块替换：与 ConfigBuilder::build 共用 build_dns_rules
        // （防两条生成路径漂移的结构性手段）。v2 门控以 route.rule_set 是否
        // 已注册 geoip-cn 为准（上方步骤3刚按文件存在性同步；无订阅态未注册
        // 时不注入 v2 链，match_response 引用未注册 rule-set 会拒载）。
        // 整块替换同时清掉了 legacy 的 www.gstatic.com 强制 local 规则。
        let new_dns_rules = crate::core::config_builder::build_dns_rules(
            &server_domains,
            route_has_geosite,
            route_has_geoip,
            settings.dns_smart_routing_v2,
            is_fake_ip,
        );
        if dns.get("rules") != Some(&serde_json::json!(new_dns_rules)) {
            dns.insert("rules".to_string(), serde_json::json!(new_dns_rules));
            modified = true;
        }
    }

    // 6. 清理 mixed inbound 上的 legacy 嗅探字段（迁移兜底）
    // sniff/sniff_override_destination 在 sing-box 1.13 已移除、1.14 拒载整份配置；
    // 旧版本生成的 config.json 可能残留，此处主动剔除（嗅探由 route.rules 的
    // {"action":"sniff"} 规则承担）
    if let Some(inbounds) = config_val
        .get_mut("inbounds")
        .and_then(|i| i.as_array_mut())
    {
        for inb in inbounds.iter_mut() {
            if let Some(obj) = inb.as_object_mut() {
                if obj.remove("sniff").is_some()
                    | obj.remove("sniff_override_destination").is_some()
                {
                    modified = true;
                }
            }
        }
    }

    if modified {
        let new_content = serde_json::to_string_pretty(&config_val).map_err(|e| {
            crate::error::AppError::Config(format!("序列化 config.json 失败: {}", e))
        })?;
        crate::fs_utils::atomic_write(&config_path, new_content.as_bytes())
            .map_err(|e| crate::error::AppError::Io(format!("写入 config.json 失败: {}", e)))?;
        log::info!(
            "[settings] 已重建 config.json，已配置端口并包含 TUN 节点: {}",
            settings.tun_enabled
        );
    }

    Ok(())
}

/// 获取所有设置
#[tauri::command]
pub async fn settings_get_all(app_handle: tauri::AppHandle) -> ApiResponse<AppSettings> {
    ApiResponse::ok(settings_get_internal(&app_handle))
}

/// 内部保存设置辅助函数（读-改-写事务）
///
/// 事务规则：
/// 1. 整个读-改-写由 acquire_persist_lock 串行化，防止托盘/前端/调度器并发覆盖
/// 2. 文件存在但读取或解析失败 → 返回错误（绝不静默用默认值整体替换，避免丢设置）
/// 3. 仅当文件不存在时才以默认值起步
/// 4. 缺 mixed_port 等单个字段时，先取 defaults，再用现有内容逐 key 覆盖
///    （方向：以现有内容覆盖默认值，而非用默认值整体替换）
pub fn update_settings_internal(
    app_handle: &tauri::AppHandle,
    patch: serde_json::Value,
) -> Result<(), String> {
    let path = get_settings_path();
    // 同步辅助函数内完成 lock → read → merge → atomic_write 全程持锁
    let merged = {
        let _guard = crate::fs_utils::acquire_persist_lock();
        let existing: serde_json::Value = if path.exists() {
            let content = fs::read_to_string(&path).map_err(|e| {
                format!("读取 settings.json 失败（为避免丢失设置已中止保存）: {}", e)
            })?;
            serde_json::from_str::<serde_json::Value>(&content).map_err(|e| {
                format!("解析 settings.json 失败（为避免丢失设置已中止保存）: {}", e)
            })?
        } else {
            serde_json::json!({})
        };

        // 合并方向修正：merged = defaults，然后以现有内容覆盖默认值，
        // 这样单个缺失字段由默认值补齐，已有字段全部保留
        let defaults = serde_json::to_value(AppSettings::default())
            .map_err(|e| format!("序列化默认设置失败: {}", e))?;
        let mut merged = defaults;
        if let (Some(m_obj), Some(e_obj)) = (merged.as_object_mut(), existing.as_object()) {
            for (k, v) in e_obj {
                m_obj.insert(k.clone(), v.clone());
            }
        }

        // 应用补丁
        if let Some(obj) = merged.as_object_mut() {
            if let Some(patch_obj) = patch.as_object() {
                for (k, v) in patch_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }

        let content =
            serde_json::to_string_pretty(&merged).map_err(|e| format!("序列化设置失败: {}", e))?;
        crate::fs_utils::atomic_write(&path, content.as_bytes())
            .map_err(|e| format!("写入 settings.json 失败: {}", e))?;
        merged
    };

    if let Ok(new_settings) = serde_json::from_value::<AppSettings>(merged) {
        crate::core::clash_api::set_clash_api_port(new_settings.clash_api_port);
    }

    rebuild_config_from_settings(app_handle).map_err(|e| format!("重建内核配置失败: {}", e))?;
    Ok(())
}

/// 将当前的 config.json 配置同步下发给系统服务
pub async fn sync_config_to_service(_app_handle: &tauri::AppHandle) -> Result<(), String> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        return Err("config.json 配置文件不存在，请先导入订阅".to_string());
    }

    // IPC 协议约定：RELOAD_CONFIG 的 config 参数一律为内嵌配置文本
    // （服务端会将其写入自己的 config.json 再拉起内核）。
    // 历史缺陷：曾把文件路径字符串当内容发送，服务端把路径本身写进
    // config.json，sing-box 拿到非 JSON 内容必然启动失败。
    let config_content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("读取 config.json 失败: {}", e))?;
    log::info!(
        "[settings] 向系统服务同步配置，内容长度: {} 字节",
        config_content.len()
    );

    let resp =
        crate::core::ipc_client::send_ipc_request("RELOAD_CONFIG", Some(&config_content)).await?;
    if !resp.success {
        return Err(resp
            .error
            .unwrap_or_else(|| "服务内部处理配置重载异常".to_string()));
    }
    Ok(())
}

/// 保存设置
///
/// 性能优化：仅当内核相关字段（mixed_port, clash_api_port, allow_lan, proxy_mode,
/// tun_enabled, run_mode）发生变化时才触发 apply_core_mode_with_fallback 重启内核，
/// 避免修改主题、性能模式等无关设置时产生不必要的内核重启。
#[tauri::command]
pub async fn settings_save(
    app_handle: tauri::AppHandle,
    patch: serde_json::Value,
) -> ApiResponse<()> {
    log::info!("[settings] 保存设置，补丁: {:?}", patch);

    // 记录保存前的设置，用于后续比较内核相关字段是否变化
    let old_settings = settings_get_internal(&app_handle);

    if let Err(e) = update_settings_internal(&app_handle, patch) {
        return ApiResponse::err(format!("写入设置失败: {}", e), 500);
    }

    // 读取保存后的设置
    let new_settings = settings_get_internal(&app_handle);

    // 判断内核相关字段是否变化
    let core_changed = old_settings.mixed_port != new_settings.mixed_port
        || old_settings.clash_api_port != new_settings.clash_api_port
        || old_settings.allow_lan != new_settings.allow_lan
        || old_settings.proxy_mode != new_settings.proxy_mode
        || old_settings.tun_enabled != new_settings.tun_enabled
        || old_settings.core.run_mode != new_settings.core.run_mode;

    // 开机自启：字段变化时同步系统注册（LaunchAgent），失败不阻塞设置保存
    if old_settings.auto_start != new_settings.auto_start {
        if let Err(e) =
            crate::system::autostart::sync_autostart(&app_handle, new_settings.auto_start)
        {
            log::error!("[settings] {}", e);
            return ApiResponse::err(e, 500);
        }
    }

    if core_changed {
        log::info!("[settings] 检测到内核相关字段变化，触发配置重载和内核重启");
        // 统一通过自愈恢复逻辑应用配置和重载内核
        if let Err(e) = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
            return ApiResponse::err(e, 500);
        }
    } else {
        log::info!("[settings] 内核相关字段未变化，跳过内核重启");
    }

    ApiResponse::ok(())
}

/// 校验注入主机名：仅允许字母、数字、点、冒号、短横线（覆盖 IPv4/IPv6/主机名）
fn is_valid_proxy_host(host: &str) -> bool {
    !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == ':' || c == '-')
}

/// 生成终端代理环境变量注入命令（开发者工具箱）
#[tauri::command]
pub async fn settings_inject_terminal_proxy(host: String, port: u16) -> ApiResponse<Vec<String>> {
    // 主机参数会被拼接进 shell 命令，必须先做字符白名单校验，防止命令注入
    if !is_valid_proxy_host(&host) {
        return ApiResponse::err(
            crate::error::AppError::Validation(format!(
                "无效的主机地址: {:?}（仅允许字母、数字、点、冒号、短横线）",
                host
            )),
            400,
        );
    }
    let proxy_url = format!("http://{}:{}", host, port);
    let commands = vec![
        format!("export http_proxy={}", proxy_url),
        format!("export https_proxy={}", proxy_url),
        format!("export all_proxy={}", proxy_url),
        format!("$env:http_proxy=\"{}\"", proxy_url),
        format!("$env:https_proxy=\"{}\"", proxy_url),
    ];
    ApiResponse::ok(commands)
}

/// 导出诊断日志（返回日志文件路径）
#[tauri::command]
pub async fn settings_export_diagnostic_log(app_handle: tauri::AppHandle) -> ApiResponse<String> {
    // 注意：日志文件由 lib.rs 中 tauri-plugin-log 写入到 get_log_dir() 目录
    // get_log_dir() = get_data_root()/logs，不能误用 get_config_dir()/logs
    let log_path = crate::get_log_dir().join("auroweave.log");

    if !log_path.exists() {
        return ApiResponse::err("诊断日志不存在，请先运行核心服务", 404);
    }

    match app_handle.path().desktop_dir() {
        Ok(desktop) => {
            let target_path = desktop.join("Auroweave_diagnostic_log.txt");
            match fs::copy(&log_path, &target_path) {
                Ok(_) => {
                    log::info!("[settings] 成功导出诊断日志至桌面: {:?}", target_path);
                    ApiResponse::ok(target_path.to_string_lossy().to_string())
                }
                Err(e) => ApiResponse::err(format!("复制日志到桌面失败: {}", e), 500),
            }
        }
        Err(e) => ApiResponse::err(format!("获取桌面路径失败: {}", e), 500),
    }
}

/// 专用 TUN 模式切换命令 (同步等待进程启动结果并将失败透传前端)
///
/// 可选 proxy_mode 参数：若提供，会同时更新代理模式，避免前端需要
/// 额外调用 proxy_set_mode 触发第二次进程重启（macOS 上即第二次密码框）。
#[tauri::command]
pub async fn tun_set_enabled(
    app_handle: tauri::AppHandle,
    enabled: bool,
    proxy_mode: Option<String>,
) -> ApiResponse<()> {
    log::info!(
        "[settings] 切换 TUN 模式状态: enabled={}, proxy_mode={:?}",
        enabled,
        proxy_mode
    );

    let mut current_settings = settings_get_internal(&app_handle);
    current_settings.tun_enabled = enabled;

    // 若提供了 proxy_mode 且合法，一并更新，避免前端二次调用 proxy_set_mode
    if let Some(ref mode) = proxy_mode {
        if ["global", "rule", "direct"].contains(&mode.as_str()) {
            current_settings.proxy_mode = mode.clone();
        }
    }

    let patch = match serde_json::to_value(current_settings) {
        Ok(v) => v,
        Err(e) => return ApiResponse::err(format!("序列化设置失败: {}", e), 500),
    };
    if let Err(e) = update_settings_internal(&app_handle, patch) {
        return ApiResponse::err(format!("保存设置失败: {}", e), 500);
    }

    // 统一通过自愈恢复逻辑应用配置和重载内核
    if let Err(e) = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
        return ApiResponse::err(e, 500);
    }

    ApiResponse::ok(())
}

/// 查询系统服务状态与版本
#[tauri::command]
pub async fn service_query_status(
    app_handle: tauri::AppHandle,
) -> ApiResponse<CoreServiceSettings> {
    let status = crate::system::service_control::query_service_status().unwrap_or_else(|e| {
        log::error!("[settings] 查询 Windows 系统服务状态异常: {}", e);
        "error".to_string()
    });

    let settings = settings_get_internal(&app_handle);
    let mut current_core = settings.core;
    current_core.service.last_known_status = status;

    ApiResponse::ok(current_core.service)
}

/// 提权安装系统服务或提权计划任务
#[tauri::command]
pub async fn service_install(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    let settings = settings_get_internal(&app_handle);
    let run_mode = settings.core.run_mode.clone();

    // 1. 定位当前有效 sing-box 路径以传给安装器
    let singbox_path = match crate::core::sidecar::SidecarManager::resolve_binary_path() {
        Ok(path) => path,
        Err(e) => {
            log::error!("[settings] 定位 sing-box 二进制失败: {:?}", e);
            return ApiResponse::err(format!("定位 sing-box 失败，请先下载: {}", e), 404);
        }
    };

    // 2. 调用 UAC 提权安装，传入 run_mode 和 sing-box 路径
    match crate::system::service_control::install_service_uac(&app_handle, &run_mode, &singbox_path)
    {
        Ok(_) => {
            let version = app_handle.package_info().version.to_string();
            let mut settings = settings_get_internal(&app_handle);
            settings.core.service.installed_version = Some(version);
            settings.core.service.last_fallback_reason = None;

            if run_mode == "service" {
                // 仅服务模式下自动启动服务，并检查启动结果
                match crate::system::service_control::start_service() {
                    Ok(_) => {
                        // 启动成功后再查询实际状态，避免硬编码 running 掩盖异步失败
                        let actual_status = crate::system::service_control::query_service_status()
                            .unwrap_or_else(|e| {
                                log::warn!("[settings] 查询服务状态失败: {}", e);
                                "unknown".to_string()
                            });
                        if actual_status == "stopped" || actual_status == "not_installed" {
                            log::error!(
                                "[settings] 服务启动指令成功但服务未进入运行状态: {}",
                                actual_status
                            );
                            return ApiResponse::err(
                                "提权安装成功，但服务未能进入运行状态，请稍后在设置页手动启动",
                                500,
                            );
                        }
                        settings.core.service.last_known_status = actual_status;
                        log::info!("[settings] 提权安装并启动系统服务成功");
                    }
                    Err(e) => {
                        log::error!("[settings] 服务模式安装后自动启动服务失败: {}", e);
                        return ApiResponse::err(
                            format!("提权安装成功，但启动系统服务失败: {}", e),
                            500,
                        );
                    }
                }
            } else {
                // 本地运行模式下，不需要启动服务，更新状态为 stopped
                settings.core.service.last_known_status = "stopped".to_string();
                log::info!(
                    "[settings] 提权安装计划任务组件成功，本地运行模式无需启动 Windows 服务"
                );
            }

            let patch = match serde_json::to_value(settings) {
                Ok(v) => v,
                Err(e) => return ApiResponse::err(format!("序列化设置失败: {}", e), 500),
            };
            if let Err(e) = update_settings_internal(&app_handle, patch) {
                return ApiResponse::err(format!("写入服务安装状态失败: {}", e), 500);
            }
            ApiResponse::ok(())
        }
        Err(e) => {
            log::error!("[settings] 提权安装提权组件失败: {}", e);
            ApiResponse::err(format!("提权安装提权组件失败: {}", e), 500)
        }
    }
}

/// 提权卸载系统服务
#[tauri::command]
pub async fn service_uninstall(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    match crate::system::service_control::uninstall_service_uac(&app_handle) {
        Ok(_) => {
            let mut settings = settings_get_internal(&app_handle);
            settings.core.service.installed_version = None;
            settings.core.service.last_known_status = "not_installed".to_string();
            let patch = match serde_json::to_value(settings) {
                Ok(v) => v,
                Err(e) => return ApiResponse::err(format!("序列化设置失败: {}", e), 500),
            };
            if let Err(e) = update_settings_internal(&app_handle, patch) {
                return ApiResponse::err(format!("写入卸载状态失败: {}", e), 500);
            }
            log::info!("[settings] 提权卸载系统服务成功");
            ApiResponse::ok(())
        }
        Err(e) => {
            log::error!("[settings] 提权卸载系统服务失败: {}", e);
            ApiResponse::err(format!("提权卸载系统服务失败: {}", e), 500)
        }
    }
}

/// 手动启动系统服务
#[tauri::command]
pub async fn service_start(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    match crate::system::service_control::start_service() {
        Ok(_) => {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if let Err(e) = sync_config_to_service(&app_handle).await {
                return ApiResponse::err(
                    format!("服务已成功拉起，但初始化内核配置失败: {}", e),
                    500,
                );
            }
            log::info!("[settings] 手动拉起系统服务成功");
            ApiResponse::ok(())
        }
        Err(e) => {
            log::error!("[settings] 启动服务失败: {}", e);
            ApiResponse::err(format!("启动服务失败: {}", e), 500)
        }
    }
}

/// 手动停止系统服务
#[tauri::command]
pub async fn service_stop() -> ApiResponse<()> {
    match crate::system::service_control::stop_service() {
        Ok(_) => {
            log::info!("[settings] 手动停止系统服务成功");
            ApiResponse::ok(())
        }
        Err(e) => {
            log::error!("[settings] 停止服务失败: {}", e);
            ApiResponse::err(format!("停止服务失败: {}", e), 500)
        }
    }
}

/// 读取系统服务运行日志
#[tauri::command]
pub async fn service_read_log() -> ApiResponse<String> {
    // 守护进程 AuroDaemon 将日志写入 ProgramData/Auroweave/logs/service.log
    // 参见 crates/auroweave-svc/src/main.rs 的 init_file_logging()
    let log_path = crate::get_log_dir().join("service.log");
    if !log_path.exists() {
        return ApiResponse::err("系统服务运行日志文件不存在".to_string(), 404);
    }
    match fs::read_to_string(log_path) {
        Ok(c) => ApiResponse::ok(c),
        Err(e) => ApiResponse::err(format!("读取系统服务日志失败: {}", e), 500),
    }
}

/// 查询内核 sing-box 是否在真正运行中
#[tauri::command]
pub async fn core_query_running(app_handle: tauri::AppHandle) -> ApiResponse<bool> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    if !config_path.exists() {
        return ApiResponse::ok(false);
    }

    let settings = settings_get_internal(&app_handle);
    if settings.core.run_mode == "service" {
        match crate::core::ipc_client::send_ipc_request("GET_STATUS", None).await {
            Ok(resp) => ApiResponse::ok(
                resp.success && (resp.status == "running" || resp.status == "starting"),
            ),
            Err(_) => ApiResponse::ok(false),
        }
    } else {
        let sidecar_manager = app_handle
            .state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>()
            .inner()
            .clone();
        let status = sidecar_manager.get_status();
        let is_running = status == crate::core::sidecar::SidecarStatus::Running
            || status == crate::core::sidecar::SidecarStatus::Starting;

        let is_tun_process_running = if settings.tun_enabled {
            // Windows: 通过计划任务检测；macOS: 检查 PID 文件与进程存活；Linux: 检查 sidecar 状态
            #[cfg(target_os = "windows")]
            {
                crate::system::service_control::query_singbox_process_running()
            }
            #[cfg(target_os = "macos")]
            {
                // 性能：sidecar 内存态已是 Running/Starting 时无需再做进程表级校验
                // （macOS TUN 同样由 SidecarManager 拉起，状态即事实源）——
                // 避免 Dashboard 每 3s 一次 sysinfo 全进程表刷新（~20-80ms/次）。
                // 仅状态显示已停但 PID 文件存在时才做 sysinfo 单 PID 校验
                // （覆盖外部拉起/状态漂移的兜底场景）。
                if is_running {
                    true
                } else {
                    let pid_file = std::env::temp_dir().join("auroweave-singbox.pid");
                    if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
                        if let Ok(pid) = pid_str.trim().parse::<u32>() {
                            let mut sys = sysinfo::System::new();
                            sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
                            sys.process(sysinfo::Pid::from_u32(pid)).is_some()
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos")))]
            {
                false
            }
        } else {
            false
        };

        ApiResponse::ok(is_running || is_tun_process_running)
    }
}

/// 从 config.backup.json 恢复配置文件并自动重启内核
#[tauri::command]
pub async fn settings_restore_backup(app_handle: tauri::AppHandle) -> ApiResponse<()> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");
    let backup_path = config_dir.join("config.backup.json");
    if !backup_path.exists() {
        return ApiResponse::err("未找到备份配置文件 (config.backup.json)".to_string(), 404);
    }

    if let Err(e) = fs::copy(&backup_path, &config_path) {
        return ApiResponse::err(format!("还原备份文件失败: {}", e), 500);
    }
    log::info!("[settings] 已成功还原 config.backup.json 至 config.json");

    // 重新应用设置覆写
    if let Err(e) = rebuild_config_from_settings(&app_handle) {
        return ApiResponse::err(format!("备份已还原，但重建配置失败: {}", e), 500);
    }

    // 重新拉起/重载内核
    if let Err(e) = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
        return ApiResponse::err(format!("备份已还原，但拉起内核失败: {}", e), 500);
    }

    ApiResponse::ok(())
}

/// 更新分组测速配置（interval / tolerance / url），持久化到 settings.json
///
/// 参数 `config` 为 JSON 字符串（前端序列化，interval 已归一为秒）：
/// `{"interval": 180, "tolerance": 50, "url": "http://..."}`，
/// 未提供的字段不覆盖已有值。保存后同步修补 config.json 中对应的
/// urltest 出站（下次重建配置时 ConfigBuilder 亦会应用）。
#[tauri::command]
pub async fn group_update_config(
    app_handle: tauri::AppHandle,
    group_tag: String,
    config: String,
) -> ApiResponse<bool> {
    // 解析并校验覆盖项
    let parsed: GroupTestConfig = match serde_json::from_str(&config) {
        Ok(v) => v,
        Err(e) => return ApiResponse::err(format!("分组配置 JSON 解析失败: {}", e), 400),
    };

    // 基本参数校验：interval 合理区间 30s~24h，tolerance 0~2000ms，url 为 http(s)
    if let Some(interval) = parsed.interval {
        if !(30..=86400).contains(&interval) {
            return ApiResponse::err(
                format!("测速间隔需在 30 秒到 24 小时之间（当前 {}s）", interval),
                400,
            );
        }
    }
    if let Some(tolerance) = parsed.tolerance {
        if tolerance > 2000 {
            return ApiResponse::err(
                format!("容差需在 0~2000ms 之间（当前 {}ms）", tolerance),
                400,
            );
        }
    }
    if let Some(url) = &parsed.url {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            match reqwest::Url::parse(trimmed) {
                Ok(u) if u.scheme() == "http" || u.scheme() == "https" => {}
                _ => {
                    return ApiResponse::err(
                        "测速 URL 必须是合法的 http/https 地址".to_string(),
                        400,
                    )
                }
            }
        }
    }

    // 读-改-写 settings.group_configs（持久化锁 + 原子写由 update_settings_internal 保证）
    let mut settings = settings_get_internal(&app_handle);
    settings.group_configs.insert(group_tag.clone(), parsed);

    let patch = serde_json::to_value(&settings).unwrap_or_else(|_| serde_json::json!({}));
    match update_settings_internal(&app_handle, patch) {
        Ok(_) => {}
        Err(e) => return ApiResponse::err(format!("保存分组配置失败: {}", e), 500),
    }

    // 同步修补当前 config.json 的对应 urltest 出站（热生效，无需等下次重建）
    if let Err(e) = rebuild_config_from_settings(&app_handle) {
        log::error!("[settings] 应用分组配置到 config.json 失败: {}", e);
        return ApiResponse::err(format!("配置已保存，但同步内核配置失败: {}", e), 500);
    }

    log::info!("[settings] 分组 [{}] 测速配置已更新并同步", group_tag);
    ApiResponse::ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_default_unified_delay_to_disabled() {
        let mut value = serde_json::to_value(AppSettings::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.remove("latency_unified_delay");
        object.remove("latency_persistent_reuse");
        let settings: AppSettings = serde_json::from_value(value).unwrap();
        assert!(!settings.latency_unified_delay);
        assert!(settings.latency_persistent_reuse);
    }

    #[test]
    fn legacy_default_speed_source_migrates_to_github() {
        let mut settings = AppSettings::default();
        settings.speed_test_url = "https://speed.cloudflare.com/__down?bytes=25000000".to_string();
        settings.speed_test_urls = vec![
            "https://speed.cloudflare.com/__down?bytes=25000000".to_string(),
            "https://fast.com".to_string(),
        ];
        apply_settings_migrations(&mut settings);
        assert!(settings.speed_test_url.contains("github.com/BurntSushi/ripgrep"));
        assert_eq!(settings.speed_test_urls.len(), 4);

        let mut custom = AppSettings::default();
        custom.speed_test_url = "https://example.com/custom.bin".to_string();
        apply_settings_migrations(&mut custom);
        assert_eq!(custom.speed_test_url, "https://example.com/custom.bin");
    }

    #[test]
    fn test_outbound_reference_helpers_follow_actual_tags() {
        let config = serde_json::json!({
            "outbounds": [
                {"type": "direct", "tag": "direct"},
                {"type": "block", "tag": "block"}
            ]
        });
        let tags = config_outbound_tags(&config);
        assert!(tags.contains("direct"));
        assert!(tags.contains("block"));
        let remote = canonical_remote_dns_server("dns.example", None);
        assert_eq!(remote["tag"], "remote");
        assert!(remote.get("detour").is_none());

        let remote_with_proxy = canonical_remote_dns_server("dns.example", Some("proxy"));
        assert_eq!(remote_with_proxy["detour"], "proxy");
    }
}
