/// Sing-box config.json 动态配置生成器
/// 作者: TanXiang
///
/// 职责：
/// 1. 整理出站节点（去重、自动按地区分类成 urltest 分组）
/// 2. 构建 DNS 路由、RuleSet 和 Inbounds（Mixed 动态监听端口）
/// 3. 配置 ClashAPI 动态监听端口
///
/// 配置生成流程：
/// 1. **节点处理**：遍历所有 ParsedOutbound，提取 tag 列表并按地区自动分组
///    - 每个地区创建一个 `urltest` 类型出站（自动测速选最优节点）
///    - 创建全局 `Auto` 选择器出站（包含所有地区分组 + 所有节点）
/// 2. **Inbounds**：创建 Mixed 入站（HTTP+SOCKS5 混合代理），监听 mixed_port
///    - 若启用 TUN，额外创建 TUN 虚拟网卡入站
/// 3. **DNS 配置**：配置国内/国外 DNS 分流，使用 fake-ip 模式
/// 4. **路由规则**：配置 RuleSet（geosite/geoip），按域名和 IP 分流
/// 5. **ClashAPI**：配置外部控制器，监听 clash_api_port
///
/// 地区检测策略（`detect_region`）：
/// - 优先匹配国旗 emoji（最精确）
/// - 其次匹配中文关键词（香港、日本、美国等）
/// - 最后使用英文缩写单词边界匹配（避免 "us" 误匹配 "Russia"）
use super::parser::{is_announcement_or_fake_node, ParsedOutbound};
use crate::error::AppError;
use serde_json::{json, Value};
use std::collections::HashMap;

/// 生成最小默认配置（无代理节点时使用）
///
/// 当用户尚未导入订阅时，生成一个仅包含 direct/block 出站的基础配置，
/// 使 sing-box 能够正常启动并监听端口，ClashAPI 也可正常访问。
/// 后续导入订阅后会通过 ConfigBuilder 重新生成完整配置。
pub fn generate_minimal_config(mixed_port: u16, clash_api_port: u16) -> Value {
    json!({
        "log": {
            "level": "info",
            "timestamp": true
        },
        "dns": {
            "servers": [
                {
                    "tag": "remote",
                    "type": "udp",
                    "server": "8.8.8.8"
                },
                {
                    "tag": "local",
                    "type": "local"
                }
            ],
            "rules": [],
            "final": "local",
            "strategy": "prefer_ipv4"
        },
        "inbounds": [
            {
                "type": "mixed",
                "tag": "mixed-in",
                "listen": "127.0.0.1",
                "listen_port": mixed_port
            }
        ],
        "outbounds": [
            { "type": "direct", "tag": "direct" },
            { "type": "block", "tag": "block" }
        ],
        "route": {
            "default_domain_resolver": "local",
            "rules": [
                { "action": "sniff" },
                { "protocol": "dns", "action": "hijack-dns" },
                { "clash_mode": "direct", "action": "route", "outbound": "direct" },
                { "ip_is_private": true, "action": "route", "outbound": "direct" }
            ],
            "final": "direct",
            "auto_detect_interface": true
        },
        "experimental": {
            "clash_api": {
                "external_controller": format!("127.0.0.1:{}", clash_api_port),
                "secret": super::clash_api::get_clash_api_secret(),
                // 与完整配置同语义：无订阅场景模式恢复与用户设置一致（否则冷启动恒为 Rule）
                "default_mode": "Rule"
            },
            "cache_file": {
                "enabled": true,
                // DNS 缓存持久化（1.14.0 新增，experimental/cache-file.md）：
                // 重启后无需重新解析域名，节点域名冷启动明显加速
                "store_dns": true,
                "path": crate::get_data_root().join("cache.db").to_string_lossy().to_string()
            }
        }
    })
}

pub struct ConfigBuilder {
    outbounds: Vec<ParsedOutbound>,
    mixed_port: u16,
    clash_api_port: u16,
    allow_lan: bool,
    /// geosite-cn.srs 本地文件路径（如果存在则使用 type:local，否则跳过 rule-set）
    geosite_cn_path: Option<String>,
    /// geoip-cn.srs 本地文件路径
    geoip_cn_path: Option<String>,
    /// 分组测速配置覆盖（group tag -> interval/tolerance/url），
    /// 由设置页 GroupEditModal 保存，仅覆盖显式设置的字段
    group_configs: std::collections::HashMap<String, crate::commands::settings::GroupTestConfig>,
    /// 远端 DoH 服务器地址（DNS 设置页；空串回退默认 8.8.8.8）
    dns_remote_doh: String,
    /// DNS 查询超时秒数（1.14.0 optimistic/timeout；timeout<=0 用默认 5s）
    dns_timeout_secs: u64,
    /// 乐观 DNS 缓存开关（过期缓存立即返回 + 后台刷新）
    dns_optimistic_cache: bool,
}

impl ConfigBuilder {
    /// 创建配置生成器，默认端口 mixed=8890, clash_api=9090
    pub fn new(outbounds: Vec<ParsedOutbound>) -> Self {
        Self {
            outbounds,
            mixed_port: 8890,
            clash_api_port: 9090,
            allow_lan: false,
            geosite_cn_path: None,
            geoip_cn_path: None,
            group_configs: std::collections::HashMap::new(),
            dns_remote_doh: String::new(),
            dns_timeout_secs: 5,
            dns_optimistic_cache: true,
        }
    }

    /// 设置局域网共享模式
    pub fn with_allow_lan(mut self, allow_lan: bool) -> Self {
        self.allow_lan = allow_lan;
        self
    }

    /// 设置 DNS 配置（远端 DoH 地址 / 查询超时秒 / 乐观缓存开关）
    pub fn with_dns(mut self, remote_doh: String, timeout_secs: u64, optimistic: bool) -> Self {
        self.dns_remote_doh = remote_doh;
        self.dns_timeout_secs = timeout_secs;
        self.dns_optimistic_cache = optimistic;
        self
    }

    /// 设置混合代理端口和 Clash API 端口（仅当传入值 > 0 时生效）
    pub fn with_ports(mut self, mixed_port: u16, clash_api_port: u16) -> Self {
        if mixed_port > 0 {
            self.mixed_port = mixed_port;
        }
        if clash_api_port > 0 {
            self.clash_api_port = clash_api_port;
        }
        self
    }

    /// 设置本地 rule-set 文件路径（如果文件存在则使用 type:local 引用）
    pub fn with_local_rule_sets(mut self, geosite_cn: Option<String>, geoip_cn: Option<String>) -> Self {
        self.geosite_cn_path = geosite_cn;
        self.geoip_cn_path = geoip_cn;
        self
    }

    /// 设置分组测速配置覆盖（GroupEditModal 保存的 interval/tolerance/url）
    pub fn with_group_configs(mut self, group_configs: std::collections::HashMap<String, crate::commands::settings::GroupTestConfig>) -> Self {
        self.group_configs = group_configs;
        self
    }

    /// 生成 urltest 出站的公共参数：应用用户对指定 group tag 的覆盖配置。
    /// 未覆盖的字段保持内置默认（interval "3m"、tolerance 50、gstatic 测速 URL）
    fn urltest_params(&self, tag: &str) -> (String, String, u64) {
        const DEFAULT_INTERVAL_SECS: u64 = 180; // "3m"
        const DEFAULT_TOLERANCE: u64 = 50;
        const DEFAULT_URL: &str = "http://www.gstatic.com/generate_204";

        match self.group_configs.get(tag) {
            Some(cfg) => (
                format!("{}s", cfg.interval.unwrap_or(DEFAULT_INTERVAL_SECS)),
                cfg.url.clone().unwrap_or_else(|| DEFAULT_URL.to_string()),
                cfg.tolerance.unwrap_or(DEFAULT_TOLERANCE),
            ),
            None => (
                DEFAULT_INTERVAL_SECS.to_string() + "s",
                DEFAULT_URL.to_string(),
                DEFAULT_TOLERANCE,
            ),
        }
    }

    /// 生成完整的 sing-box 1.11+ / 1.13+ / 1.14+ 兼容 config.json
    pub fn build(&self) -> Result<Value, AppError> {
        if self.outbounds.is_empty() {
            return Err(AppError::Config("没有可用节点，无法生成 config.json".to_string()));
        }

        // ---- 阶段1: 遍历节点，提取 tag 和原始 JSON，按地区分组（过滤公告和伪节点） ----
        // 节点 tag 去重：sing-box 要求 outbound tag 全局唯一，重名节点追加 -2/-3 后缀
        // 保留名冲突：与 direct/block/proxy/auto/balance/{region}-auto 冲突的节点跳过（log::warn）
        const RESERVED_TAGS: &[&str] = &["direct", "block", "proxy", "auto", "balance"];

        let mut valid_node_tags = Vec::new();
        let mut all_node_tags = Vec::new();
        let mut raw_outbounds = Vec::new();
        let mut region_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut seen_tags: std::collections::HashSet<String> = std::collections::HashSet::new();

        for out in &self.outbounds {
            let mut tag = out.tag.clone();
            let lower_tag = tag.to_lowercase();

            // 与保留 tag 冲突的节点跳过（selector/urltest 会引用同名 tag 造成循环）
            if RESERVED_TAGS.contains(&lower_tag.as_str()) || lower_tag.ends_with("-auto") {
                log::warn!("[config] 节点 tag [{}] 与保留名冲突，已跳过该节点", tag);
                continue;
            }

            // 重名节点追加 -2/-3 后缀去重
            if !seen_tags.insert(tag.clone()) {
                let mut suffix = 2u32;
                let mut candidate = format!("{}-{}", tag, suffix);
                while !seen_tags.insert(candidate.clone()) {
                    suffix += 1;
                    candidate = format!("{}-{}", tag, suffix);
                }
                log::warn!("[config] 节点 tag [{}] 重复，已重命名为 [{}]", tag, candidate);
                tag = candidate;
            }

            let mut raw_json = out.raw_json.clone();
            raw_json["tag"] = json!(tag);
            all_node_tags.push(tag.clone());
            raw_outbounds.push(raw_json);

            // 过滤伪节点/公告节点，避免污染自动测速策略组
            let is_fake = is_announcement_or_fake_node(&out.tag, out.server.as_deref(), out.server_port);
            if !is_fake {
                valid_node_tags.push(tag.clone());
                let region = detect_region(&tag);
                region_map.entry(region).or_default().push(tag.clone());
            }
        }

        if all_node_tags.is_empty() {
            return Err(AppError::Config("所有节点 tag 均与保留名冲突，无法生成 config.json".to_string()));
        }

        // 如果全部都是伪节点（极端情况），回退使用全部 tag
        let pool_tags = if valid_node_tags.is_empty() {
            all_node_tags.clone()
        } else {
            valid_node_tags.clone()
        };
        let valid_node_tag_set: std::collections::HashSet<String> = valid_node_tags.iter().cloned().collect();

        let mut final_outbounds = Vec::new();

        // ---- 阶段2: 构建出站列表 ----

        // 2a. Direct 与 Block 基础出站
        final_outbounds.push(json!({ "type": "direct", "tag": "direct" }));
        final_outbounds.push(json!({ "type": "block", "tag": "block" }));

        // 2b. Selector "proxy" 主出站
        // region_map 迭代前按 region 名排序，保证生成配置的确定性（HashMap 迭代顺序不稳定）
        let mut sorted_regions: Vec<String> = region_map.keys().cloned().collect();
        sorted_regions.sort();
        let mut proxy_group_list = vec!["auto".to_string(), "balance".to_string()];
        for region in &sorted_regions {
            proxy_group_list.push(format!("{}-auto", region));
        }
        // 先放入有效真实节点，再追加其它节点（HashSet O(1) 查询）
        proxy_group_list.extend(pool_tags.clone());
        if !valid_node_tags.is_empty() {
            for tag in &all_node_tags {
                if !valid_node_tag_set.contains(tag) {
                    proxy_group_list.push(tag.clone());
                }
            }
        }

        final_outbounds.push(json!({
            "type": "selector",
            "tag": "proxy",
            "outbounds": proxy_group_list
        }));

        // 2c. 全局 "auto" urltest 出站（默认 3 分钟心跳，50ms 容差，可被用户覆盖）
        let (auto_interval, auto_url, auto_tolerance) = self.urltest_params("auto");
        final_outbounds.push(json!({
            "type": "urltest",
            "tag": "auto",
            "outbounds": pool_tags.clone(),
            "url": auto_url,
            "interval": auto_interval,
            "idle_timeout": "15m",
            "tolerance": auto_tolerance,
            "interrupt_exist_connections": false
        }));

        // 2c-2. "balance" 负载均衡出站（urltest + 短间隔 + tolerance，可被用户覆盖）
        let (bal_interval, bal_url, bal_tolerance) = self.urltest_params("balance");
        final_outbounds.push(json!({
            "type": "urltest",
            "tag": "balance",
            "outbounds": pool_tags,
            "url": bal_url,
            "interval": bal_interval,
            "idle_timeout": "10m",
            "tolerance": bal_tolerance,
            "interrupt_exist_connections": false
        }));

        // 2d. 地区 urltest 出站（默认 3 分钟自动探测最优节点，按 region 名排序保证确定性）
        for region in &sorted_regions {
            let region_tag = format!("{}-auto", region);
            let (rg_interval, rg_url, rg_tolerance) = self.urltest_params(&region_tag);
            final_outbounds.push(json!({
                "type": "urltest",
                "tag": region_tag,
                "outbounds": region_map[region].clone(),
                "url": rg_url,
                "interval": rg_interval,
                "idle_timeout": "15m",
                "tolerance": rg_tolerance,
                "interrupt_exist_connections": false
            }));
        }

        // 2e. 节点具体出站
        final_outbounds.extend(raw_outbounds);

        // ---- 阶段3: 构建 DNS 规则 ----
        let mut server_domains = Vec::new();
        for out in &self.outbounds {
            if let Some(server) = out.raw_json.get("server").and_then(|s| s.as_str()) {
                if server.parse::<std::net::IpAddr>().is_err() && !server.is_empty() {
                    server_domains.push(server.to_string());
                }
            }
        }
        server_domains.sort();
        server_domains.dedup();

        let mut dns_rules = Vec::new();
        // Direct 模式全量本地解析（与 route 的 clash_mode 单一真相源对齐）：
        // TUN 下系统 DNS 劫持的查询在 Direct 模式走 local，不依赖 remote DoH
        //（detour "proxy" 引用不经过 route.rules，Direct 前置规则对 DoH 无效，
        //  节点故障时直连模式域名解析不应随之失败）
        dns_rules.push(json!({ "clash_mode": "Direct", "action": "route", "server": "local" }));
        if !server_domains.is_empty() {
            dns_rules.push(json!({ "domain": server_domains, "action": "route", "server": "local" }));
        }

        // ---- 阶段4: 组装路由 ----
        // 检查本地 rule-set 文件是否存在（以 Path::exists() 实际检查为准）
        // DNS 分流与 route 规则统一使用同一组 has_geosite/has_geoip 布尔值，
        // 避免 DNS 引用 geosite-cn 而 route 未注册该 rule-set 的不一致门控
        let has_geosite = self.geosite_cn_path.as_ref().map(|p| std::path::Path::new(p).exists()).unwrap_or(false);
        let has_geoip = self.geoip_cn_path.as_ref().map(|p| std::path::Path::new(p).exists()).unwrap_or(false);

        // 仅当 geosite-cn rule-set 可用时才添加 DNS 规则：国内域名由 local DNS 权威解析
        if has_geosite {
            dns_rules.push(json!({ "rule_set": "geosite-cn", "action": "route", "server": "local" }));
        }

        let mut rule_set_config = Vec::new();
        // geosite 与 geoip 各自独立注册（与 build_full_route_rules 的独立布尔语义对齐）
        if has_geosite {
            rule_set_config.push(json!({
                "tag": "geosite-cn",
                "type": "local",
                "format": "binary",
                "path": self.geosite_cn_path.as_ref().unwrap()
            }));
        }
        if has_geoip {
            rule_set_config.push(json!({
                "tag": "geoip-cn",
                "type": "local",
                "format": "binary",
                "path": self.geoip_cn_path.as_ref().unwrap()
            }));
        }
        if rule_set_config.is_empty() {
            log::warn!("[config] geosite-cn.srs / geoip-cn.srs 本地文件均不存在，跳过国内直连规则，所有流量走代理");
        }

        let route_rules = build_full_route_rules(has_geosite, has_geoip);

        let route = if rule_set_config.is_empty() {
            json!({
                "default_domain_resolver": "local",
                "rules": route_rules,
                "final": "proxy",
                "auto_detect_interface": true
            })
        } else {
            json!({
                "default_domain_resolver": "local",
                "rule_set": rule_set_config,
                "rules": route_rules,
                "final": "proxy",
                "auto_detect_interface": true
            })
        };

        // ---- 阶段5: 组装最终 JSON ----
        let config = json!({
            "log": {
                "level": "info",
                "timestamp": true
            },
            "dns": {
                "servers": [
                    {
                        "detour": "proxy",
                        "server": if self.dns_remote_doh.trim().is_empty() { "8.8.8.8" } else { self.dns_remote_doh.trim() },
                        "tag": "remote",
                        "type": "https"
                    },
                    {
                        "tag": "local",
                        "type": "local",
                        // 1.14.0：单标签与 .lan/.local 后缀域名走系统邻居解析器
                        // （TUN 接管后 nas/打印机/HomePod 等局域网主机名仍可解析）
                        "neighbor_domain": [".", ".lan", ".local"]
                    }
                ],
                "rules": dns_rules,
                "final": "remote",
                "strategy": "prefer_ipv4",
                // 1.14.0：乐观缓存（过期立即返回+后台刷新）与查询超时（快速失败）
                "optimistic": self.dns_optimistic_cache,
                "timeout": format!("{}s", self.dns_timeout_secs.max(1))
            },
            "inbounds": [
                {
                    "type": "mixed",
                    "tag": "mixed-in",
                    "listen": if self.allow_lan { "0.0.0.0" } else { "127.0.0.1" },
                    "listen_port": self.mixed_port
                    // 注意：严禁在此添加 sniff/sniff_override_destination ——
                    // 这两个入站字段在 sing-box 1.13 已移除，1.14 直接拒载整份配置；
                    // 嗅探由 route.rules 首条 {"action":"sniff"} 承担
                }
            ],
            "outbounds": final_outbounds,
            "route": route,
            "experimental": {
                "clash_api": {
                    "external_controller": format!("127.0.0.1:{}", self.clash_api_port),
                    "secret": super::clash_api::get_clash_api_secret()
                },
                "cache_file": {
                    "enabled": true,
                    // DNS 缓存持久化（1.14.0 新增，experimental/cache-file.md）
                    "store_dns": true,
                    "path": crate::get_data_root().join("cache.db").to_string_lossy().to_string()
                }
            }
        });

        Ok(config)
    }
}

/// 识别节点 tag 属于哪个地区 (HK, JP, US, TW, SG, KR, OTHER)
fn detect_region(tag: &str) -> String {
    let lower = tag.to_lowercase();

    if tag.contains("🇭🇰") { return "HK".to_string(); }
    if tag.contains("🇯🇵") { return "JP".to_string(); }
    if tag.contains("🇺🇸") { return "US".to_string(); }
    if tag.contains("🇹🇼") { return "TW".to_string(); }
    if tag.contains("🇸🇬") { return "SG".to_string(); }
    if tag.contains("🇰🇷") { return "KR".to_string(); }

    if lower.contains("香港") { return "HK".to_string(); }
    if lower.contains("日本") { return "JP".to_string(); }
    if lower.contains("美国") || lower.contains("美國") { return "US".to_string(); }
    if lower.contains("台湾") || lower.contains("臺灣") || lower.contains("台灣") { return "TW".to_string(); }
    if lower.contains("新加坡") { return "SG".to_string(); }
    if lower.contains("韩国") || lower.contains("韓國") { return "KR".to_string(); }

    let tokens = tokenize_tag(&lower);
    if tokens.iter().any(|&t| t == "hk" || t == "hongkong" || t == "hong") { return "HK".to_string(); }
    if tokens.iter().any(|&t| t == "jp" || t == "japan") { return "JP".to_string(); }
    if tokens.iter().any(|&t| t == "us" || t == "usa" || t == "united" || t == "america") { return "US".to_string(); }
    if tokens.iter().any(|&t| t == "tw" || t == "taiwan") { return "TW".to_string(); }
    if tokens.iter().any(|&t| t == "sg" || t == "singapore") { return "SG".to_string(); }
    if tokens.iter().any(|&t| t == "kr" || t == "korea") { return "KR".to_string(); }

    "OTHER".to_string()
}

fn tokenize_tag(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 构建完整的 route.rules 规则列表（公共函数，供 ConfigBuilder 和 rebuild_config_from_settings 统一调用）
///
/// 规则匹配顺序（严格遵循域名优先、IP后置原则）：
/// 1. 基础嗅探与 DNS 劫持
/// 2. App-Matrix 应用进程分流规则
/// 3. Custom Rules 自定义域名分流规则
/// 4. 国内域名规则集直连（geosite-cn -> direct，避免境外域名提前触发国内 DNS 解析遭受污染）
/// 5. 私有内网 IP 直连（ip_is_private -> direct）
/// 6. Custom Rules 自定义 IP 分流规则 (ip_cidr)
/// 7. 国内 IP 规则集直连（geoip-cn -> direct）
/// 8. 兜底策略由 route.final 决定（Rule/Global 模式下为 proxy）
///
/// 模式门控语义（有意设计）：
/// - geosite/geoip 通用规则带 clash_mode:"rule" —— Global 模式下整体失效（全局代理=不用智能分流）
/// - App-Matrix / Custom Rules 用户显式规则不门控 —— 任何模式下都尊重用户意图
///   （与 Clash Verge Rev 的 Global 行为一致：显式规则优先于模式）
/// - process_name 规则仅在 TUN 接管下可命中（系统代理下源进程是 sing-box 自身），
///   前端 AppMatrixList 已做模式提示
pub fn build_full_route_rules(has_geosite: bool, has_geoip: bool) -> Vec<Value> {
    let mut rules = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        // Direct 模式全量直连（clash_mode 单一真相源：运行时 PATCH mode 与
        // 重启后 default_mode 行为一致；此规则须在所有分流规则之前）
        json!({ "clash_mode": "direct", "action": "route", "outbound": "direct" }),
    ];

    // 1. 注入 App-Matrix 应用分流规则 (优先级高于通用域名分流)
    let app_rules = crate::commands::routing::load_app_rules_internal();
    for (proc_name, outbound) in app_rules {
        if !proc_name.is_empty() && !outbound.is_empty() {
            rules.push(json!({
                "process_name": [proc_name],
                "outbound": outbound
            }));
        }
    }

    // 2. 注入自定义分流规则 (先处理域名规则，IP规则暂存后续注入)
    let custom_rules = crate::commands::routing::load_custom_rules_internal();
    let mut custom_ip_rules = Vec::new();

    for cr in custom_rules {
        if !cr.enabled || cr.payload.trim().is_empty() || cr.outbound_tag.trim().is_empty() {
            continue;
        }
        let payload = cr.payload.trim().to_string();
        let outbound = cr.outbound_tag.trim().to_string();

        match cr.rule_type.as_str() {
            "domain" => {
                rules.push(json!({ "domain": [payload], "outbound": outbound }));
            }
            "domain_suffix" => {
                rules.push(json!({ "domain_suffix": [payload], "outbound": outbound }));
            }
            "domain_keyword" => {
                rules.push(json!({ "domain_keyword": [payload], "outbound": outbound }));
            }
            "domain_regex" => {
                rules.push(json!({ "domain_regex": [payload], "outbound": outbound }));
            }
            "ip_cidr" => {
                custom_ip_rules.push(json!({ "ip_cidr": [payload], "outbound": outbound }));
            }
            _ => {
                rules.push(json!({ "domain_suffix": [payload], "outbound": outbound }));
            }
        }
    }

    // 3. 注入国内域名规则集直连规则 (域名优先匹配，避免未匹配的境外域名提前进行本地 DNS 解析遭投毒)
    if has_geosite {
        rules.push(json!({
            "clash_mode": "rule",
            "rule_set": ["geosite-cn"],
            "outbound": "direct"
        }));
    }

    // 4. 私有内网 IP 直连规则 (放置在域名规则之后)
    rules.push(json!({
        "ip_is_private": true,
        "outbound": "direct"
    }));

    // 5. 注入自定义 IP CIDR 规则
    rules.extend(custom_ip_rules);

    // 6. 注入国内 IP 规则集直连规则 (geoip-cn)
    if has_geoip {
        rules.push(json!({
            "clash_mode": "rule",
            "rule_set": ["geoip-cn"],
            "outbound": "direct"
        }));
    }

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_full_route_rules_order() {
        let rules = build_full_route_rules(true, true);
        assert!(!rules.is_empty());

        // 验证域名规则 (geosite-cn) 在私有 IP 规则 (ip_is_private) 之前
        let geosite_idx = rules.iter().position(|r| {
            r.get("rule_set").and_then(|rs| rs.as_array())
                .map(|arr| arr.iter().any(|item| item.as_str() == Some("geosite-cn")))
                .unwrap_or(false)
        });
        let ip_private_idx = rules.iter().position(|r| {
            r.get("ip_is_private").and_then(|v| v.as_bool()).unwrap_or(false)
        });

        assert!(geosite_idx.is_some());
        assert!(ip_private_idx.is_some());
        assert!(geosite_idx.unwrap() < ip_private_idx.unwrap(), "geosite-cn 规则必须排在 ip_is_private 规则之前");
    }

    #[test]
    fn test_config_builder_dns_and_fake_node_filtering() {
        let outbounds = vec![
            ParsedOutbound {
                tag: "认准官网地址".to_string(),
                r#type: "vless".to_string(),
                server: Some("zf-tw1.9999231.xyz".to_string()),
                server_port: Some(500),
                raw_json: json!({ "type": "vless", "tag": "认准官网地址", "server": "zf-tw1.9999231.xyz", "server_port": 500, "uuid": "xxx" }),
            },
            ParsedOutbound {
                tag: "🇯🇵 日本-极速-001".to_string(),
                r#type: "vless".to_string(),
                server: Some("zf-jp.9999231.xyz".to_string()),
                server_port: Some(1004),
                raw_json: json!({ "type": "vless", "tag": "🇯🇵 日本-极速-001", "server": "zf-jp.9999231.xyz", "server_port": 1004, "uuid": "xxx" }),
            },
            ParsedOutbound {
                tag: "🇭🇰 香港-极速-001".to_string(),
                r#type: "vless".to_string(),
                server: Some("zf-hk.9999231.xyz".to_string()),
                server_port: Some(1001),
                raw_json: json!({ "type": "vless", "tag": "🇭🇰 香港-极速-001", "server": "zf-hk.9999231.xyz", "server_port": 1001, "uuid": "xxx" }),
            },
        ];

        let builder = ConfigBuilder::new(outbounds);
        let config = builder.build().expect("build config 应该成功");

        // 验证 DNS 配置
        let dns = config.get("dns").expect("应该包含 dns 配置");
        let servers = dns.get("servers").and_then(|s| s.as_array()).expect("servers 应该存在");
        let local_srv = servers.iter().find(|s| s.get("tag").and_then(|t| t.as_str()) == Some("local")).expect("应该包含 local dns");
        assert_eq!(local_srv.get("type").and_then(|t| t.as_str()), Some("local"));
        assert!(local_srv.get("server").is_none(), "type: local 不应有 server 字段");
        // 1.14.0 neighbor_domain：局域网单标签/.lan/.local 域名走系统邻居解析器
        assert_eq!(
            local_srv.get("neighbor_domain").and_then(|n| n.as_array()).map(|a| a.len()),
            Some(3),
            "local dns 应包含 neighbor_domain 3 项"
        );

        // Direct 模式 DNS 规则必须前置（TUN 下劫持的查询在 Direct 模式走 local，
        // 不依赖 detour=proxy 的 remote DoH——节点故障时直连解析不受影响）
        let dns_rules = dns.get("rules").and_then(|r| r.as_array()).expect("dns rules 应存在");
        assert!(!dns_rules.is_empty());
        assert_eq!(
            dns_rules[0].get("clash_mode").and_then(|m| m.as_str()),
            Some("Direct"),
            "dns.rules 首条必须是 clash_mode:Direct→local"
        );
        assert_eq!(
            dns_rules[0].get("server").and_then(|s| s.as_str()),
            Some("local")
        );

        // 验证 auto urltest 策略组中过滤了公告伪节点
        let outbounds_arr = config.get("outbounds").and_then(|o| o.as_array()).expect("outbounds 应该存在");
        let auto_group = outbounds_arr.iter().find(|o| o.get("tag").and_then(|t| t.as_str()) == Some("auto")).expect("应该包含 auto 分组");
        let auto_nodes = auto_group.get("outbounds").and_then(|n| n.as_array()).expect("auto 节点列表");
        let auto_tags: Vec<String> = auto_nodes.iter().filter_map(|s| s.as_str().map(|v| v.to_string())).collect();

        assert!(!auto_tags.contains(&"认准官网地址".to_string()), "auto 分组不应包含公告节点");
        assert!(auto_tags.contains(&"🇯🇵 日本-极速-001".to_string()), "auto 分组应包含有效日本节点");
        assert!(auto_tags.contains(&"🇭🇰 香港-极速-001".to_string()), "auto 分组应包含有效香港节点");
    }

    /// 构造测试用节点的辅助函数
    fn make_node(tag: &str) -> ParsedOutbound {
        ParsedOutbound {
            tag: tag.to_string(),
            r#type: "vless".to_string(),
            server: Some(format!("srv-{}.example.com", tag.len())),
            server_port: Some(443),
            raw_json: json!({ "type": "vless", "tag": tag, "server": "srv.example.com", "server_port": 443, "uuid": "xxx" }),
        }
    }

    #[test]
    fn test_tag_dedup_rename_suffix() {
        // 重名节点应追加 -2/-3 后缀去重，outbound tag 全局唯一
        let outbounds = vec![
            make_node("🇯🇵 日本-A"),
            make_node("🇯🇵 日本-A"),
            make_node("🇯🇵 日本-A"),
        ];
        let builder = ConfigBuilder::new(outbounds);
        let config = builder.build().expect("build config 应该成功");

        let outbound_tags: Vec<String> = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|o| o.get("tag").and_then(|t| t.as_str()).map(|s| s.to_string()))
            .collect();

        assert!(outbound_tags.contains(&"🇯🇵 日本-A".to_string()));
        assert!(outbound_tags.contains(&"🇯🇵 日本-A-2".to_string()));
        assert!(outbound_tags.contains(&"🇯🇵 日本-A-3".to_string()));

        // tag 全局唯一
        let mut sorted = outbound_tags.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), outbound_tags.len(), "outbound tag 必须全局唯一");

        // urltest 分组引用的 tag 必须与实际节点 tag 一致（重命名后同步）
        let auto_group = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o.get("tag").and_then(|t| t.as_str()) == Some("auto"))
            .unwrap();
        let auto_refs: Vec<String> = auto_group["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t.as_str().map(|s| s.to_string()))
            .collect();
        assert!(auto_refs.contains(&"🇯🇵 日本-A-2".to_string()), "urltest 引用需同步重命名后的 tag");
    }

    #[test]
    fn test_reserved_tag_conflict_skipped() {
        // 与保留名 direct/block/proxy/auto/balance/{region}-auto 冲突的节点应被跳过
        let outbounds = vec![
            make_node("direct"),
            make_node("auto"),
            make_node("HK-auto"),
            make_node("🇭🇰 香港-真实-001"),
        ];
        let builder = ConfigBuilder::new(outbounds);
        let config = builder.build().expect("build config 应该成功");

        let outbound_tags: Vec<&str> = config["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|o| o.get("tag").and_then(|t| t.as_str()))
            .collect();

        // direct/auto 只应出现一次（内置出站），冲突节点不会产生同名第二份
        assert_eq!(outbound_tags.iter().filter(|t| **t == "direct").count(), 1);
        assert_eq!(outbound_tags.iter().filter(|t| **t == "auto").count(), 1);
        // "HK-auto" 节点被跳过，不会与地区 urltest 分组 tag 冲突
        let hk_auto_count = outbound_tags.iter().filter(|t| **t == "HK-auto").count();
        assert!(hk_auto_count <= 1, "HK-auto 若存在只能是 urltest 分组，不能有同名节点出站");
        // 真实节点保留
        assert!(outbound_tags.contains(&"🇭🇰 香港-真实-001"));
    }

    #[test]
    fn test_dns_route_gating_consistent() {
        // geosite_cn_path 指向不存在的文件时：DNS 规则与 route rule-set 均不引用 geosite-cn
        let outbounds = vec![make_node("🇯🇵 日本-001")];
        let builder = ConfigBuilder::new(outbounds)
            .with_local_rule_sets(Some("/nonexistent/geosite-cn.srs".to_string()), Some("/nonexistent/geoip-cn.srs".to_string()));
        let config = builder.build().expect("build config 应该成功");

        let dns_rules = config["dns"]["rules"].as_array().unwrap();
        let dns_refs_geosite = dns_rules.iter().any(|r| {
            r.get("rule_set").and_then(|rs| rs.as_array())
                .map(|arr| arr.iter().any(|i| i.as_str() == Some("geosite-cn")))
                .unwrap_or(false)
                || r.get("rule_set").and_then(|rs| rs.as_str()) == Some("geosite-cn")
        });
        let route_rule_sets: Vec<&str> = config["route"].get("rule_set")
            .and_then(|rs| rs.as_array())
            .map(|arr| arr.iter().filter_map(|i| i.as_str()).collect())
            .unwrap_or_default();

        assert!(!dns_refs_geosite, "文件不存在时 DNS 不得引用 geosite-cn");
        assert!(!route_rule_sets.contains(&"geosite-cn"), "文件不存在时 route 不得注册 geosite-cn");
    }
}


