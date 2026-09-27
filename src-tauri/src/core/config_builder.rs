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
        // $schema 注入（与 build() 同步，plan-Q A.4-3）：三条生成路径统一
        "$schema": "https://sing-box.sagernet.org/schema.json",
        "log": {
            // P1 修复：370 节点规模下 info 级会为每条连接输出一行
            // （实测 logs/singbox.log 达 1.7MB/13942 行，2MB 即轮转），
            // 且每行在 sidecar.rs 触发一次 open/write/close 系统调用。
            // 排查问题时可临时改回 info。
            "level": "warn",
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
            // 与完整配置同语义：Direct 模式下劫持查询走本地解析
            "rules": [
                { "clash_mode": "Direct", "action": "route", "server": "local" }
            ],
            "final": "local",
            "strategy": "prefer_ipv4"
        },
        "inbounds": [
            {
                "type": "mixed",
                "tag": "mixed-in",
                "listen": "127.0.0.1",
                "listen_port": mixed_port,
                // P2 优化：与完整配置同值（TCP Fast Open）
                "tcp_fast_open": true
            }
        ],
        "outbounds": [
            // P0：移除 route.default_domain_resolver 后 direct 必须自带解析器，
            // 否则 1.14 命中 missing-domain-resolver 弃用开关直接 FATAL。
            { "type": "direct", "tag": "direct", "domain_resolver": "local" },
            { "type": "block", "tag": "block" }
        ],
        "route": {
            "rules": [
                // P2 优化：限定嗅探协议范围。默认全开会额外跑 bittorrent/rdp/
                // ssh/dtls/ntp 嗅探器（route/sniff.md 共 11 种），对分流无贡献。
                { "action": "sniff", "sniffer": ["http", "tls", "quic", "dns"] },
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
    /// geosite-cn.srs 本地缓存路径（存在时作为 remote rule-set 的
    /// initial_path——内核启动直接加载本地、后台按 url 自动更新）
    geosite_cn_path: Option<String>,
    /// geoip-cn.srs 本地缓存路径
    geoip_cn_path: Option<String>,
    /// 分组测速配置覆盖（group tag -> interval/tolerance/url），
    /// 由设置页 GroupEditModal 保存，仅覆盖显式设置的字段
    group_configs: std::collections::HashMap<String, crate::commands::settings::GroupTestConfig>,
    /// 自定义分组规则（前端 CustomGroupRule 的 JSON 形态；settings.custom_group_rules）
    custom_group_rules: Vec<Value>,
    /// 节点最近一次解锁检测状态（node_tag → {"gemini":"yes",...}），unlock 匹配用
    unlock_state: std::collections::HashMap<String, Value>,
    /// 远端 DoH 服务器地址（DNS 设置页；空串回退默认 8.8.8.8）
    dns_remote_doh: String,
    /// 节点域名解析专用直连 DoH（bootstrap；空串回退默认 223.5.5.5）
    dns_bootstrap_doh: String,
    /// bootstrap 备用直连 DoH（空串回退默认 1.12.12.12；异构运营商对冲负缓存毒化）
    dns_bootstrap_backup_doh: String,
    /// DNS 查询超时秒数（1.14.0 optimistic/timeout；timeout<=0 用默认 5s）
    dns_timeout_secs: u64,
    /// 乐观 DNS 缓存开关（过期缓存立即返回 + 后台刷新）
    dns_optimistic_cache: bool,
    /// 智能分流 v2（evaluate/match_response/respond 响应级分流，plan-P）
    dns_smart_routing_v2: bool,
    /// DNS 解析模式："fakeip"（默认）或 "realip"
    dns_mode: String,
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
            custom_group_rules: Vec::new(),
            unlock_state: std::collections::HashMap::new(),
            dns_remote_doh: String::new(),
            dns_bootstrap_doh: String::new(),
            dns_bootstrap_backup_doh: String::new(),
            dns_timeout_secs: 5,
            dns_optimistic_cache: true,
            dns_smart_routing_v2: false,
            dns_mode: "fakeip".to_string(),
        }
    }

    /// 设置局域网共享模式
    pub fn with_allow_lan(mut self, allow_lan: bool) -> Self {
        self.allow_lan = allow_lan;
        self
    }

    /// 设置 DNS 配置（远端 DoH 地址 / 查询超时秒 / 乐观缓存开关 / 智能分流 v2 开关）
    pub fn with_dns(
        mut self,
        remote_doh: String,
        timeout_secs: u64,
        optimistic: bool,
        smart_v2: bool,
    ) -> Self {
        self.dns_remote_doh = remote_doh;
        self.dns_timeout_secs = timeout_secs;
        self.dns_optimistic_cache = optimistic;
        self.dns_smart_routing_v2 = smart_v2;
        self
    }

    /// 设置 DNS 解析模式："fakeip"（默认）或 "realip"
    pub fn with_dns_mode<S: Into<String>>(mut self, dns_mode: S) -> Self {
        let mode = dns_mode.into();
        if !mode.trim().is_empty() {
            self.dns_mode = mode;
        }
        self
    }

    /// 设置节点域名解析专用直连 DoH（bootstrap；空串回退默认 223.5.5.5）
    pub fn with_bootstrap_doh(mut self, addr: String) -> Self {
        self.dns_bootstrap_doh = addr;
        self
    }

    /// 设置 bootstrap 备用直连 DoH（空串回退默认 1.12.12.12）
    pub fn with_bootstrap_backup_doh(mut self, addr: String) -> Self {
        self.dns_bootstrap_backup_doh = addr;
        self
    }

    /// 设置自定义分组规则（selector/urltest/balance 类型生成内核真实策略组）
    pub fn with_custom_groups(mut self, rules: Vec<Value>) -> Self {
        self.custom_group_rules = rules;
        self
    }

    /// 设置节点解锁检测状态（unlock 匹配用；来自 stats_db::get_latest_unlock_per_node）
    pub fn with_unlock_state(mut self, state: std::collections::HashMap<String, Value>) -> Self {
        self.unlock_state = state;
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
    pub fn with_local_rule_sets(
        mut self,
        geosite_cn: Option<String>,
        geoip_cn: Option<String>,
    ) -> Self {
        self.geosite_cn_path = geosite_cn;
        self.geoip_cn_path = geoip_cn;
        self
    }

    /// 设置分组测速配置覆盖（GroupEditModal 保存的 interval/tolerance/url）
    pub fn with_group_configs(
        mut self,
        group_configs: std::collections::HashMap<
            String,
            crate::commands::settings::GroupTestConfig,
        >,
    ) -> Self {
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
            return Err(AppError::Config(
                "没有可用节点，无法生成 config.json".to_string(),
            ));
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
                log::warn!(
                    "[config] 节点 tag [{}] 重复，已重命名为 [{}]",
                    tag,
                    candidate
                );
                tag = candidate;
            }

            let mut raw_json = out.raw_json.clone();
            raw_json["tag"] = json!(tag);
            // P0 修复（2026-09-27）：显式绑定 domain_resolver=bootstrap。
            //
            // 背景：1.14 中 `route.default_domain_resolver` 一旦指定 tag，出站解析会被
            // dialer 固定绑定到该 DNS transport 并**完全绕过 dns.rules**
            // （common/dialer/dialer.go → resolve.go initServer() 填 queryOptions.Transport，
            //  dns/router.go Lookup 走 `if options.Transport != nil` 分支直接查，不进规则匹配）。
            // 因此原先挂在 dns.rules 的节点域名主备对冲链在出站路径上从未生效，
            // 节点域名实际由系统 local 解析器解析（最需要防污染的一类域名反而裸奔）。
            //
            // 修复：给每个节点出站显式指定 bootstrap（直连 DoH，绕开运营商递归与
            // proxy 回环），并移除 route.default_domain_resolver（见 build() 阶段4/5）。
            // bootstrap 服务器自身若填域名，canonical_direct_doh_server 已挂
            // domain_resolver=local，不会形成解析环。
            raw_json["domain_resolver"] = json!("bootstrap");
            all_node_tags.push(tag.clone());
            raw_outbounds.push(raw_json);

            // 过滤伪节点/公告节点，避免污染自动测速策略组
            let is_fake =
                is_announcement_or_fake_node(&out.tag, out.server.as_deref(), out.server_port);
            if !is_fake {
                valid_node_tags.push(tag.clone());
                let region = detect_region(&tag);
                region_map.entry(region).or_default().push(tag.clone());
            }
        }

        if all_node_tags.is_empty() {
            return Err(AppError::Config(
                "所有节点 tag 均与保留名冲突，无法生成 config.json".to_string(),
            ));
        }

        // 如果全部都是伪节点（极端情况），回退使用全部 tag
        let pool_tags = if valid_node_tags.is_empty() {
            all_node_tags.clone()
        } else {
            valid_node_tags.clone()
        };
        let valid_node_tag_set: std::collections::HashSet<String> =
            valid_node_tags.iter().cloned().collect();

        let mut final_outbounds = Vec::new();

        // ---- 阶段2: 构建出站列表 ----

        // 2a. Direct 与 Block 基础出站
        // P0 修复：direct 也显式绑定 domain_resolver=local。移除
        // route.default_domain_resolver 后，任何 server 为域名的出站若无显式解析器
        // 会命中 1.14 的 missing-domain-resolver 弃用开关并直接 FATAL 拒载
        // （实测：ENABLE_DEPRECATED_MISSING_DOMAIN_RESOLVER 未设时启动失败）。
        final_outbounds.push(json!({ "type": "direct", "tag": "direct", "domain_resolver": "local" }));
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

        // 2c-2. "balance" 组：P1 修复（2026-09-27）——由 urltest 改为 selector。
        //
        // 改前问题：balance 与 auto 的 outbounds 成员列表**逐元素完全相同**
        // （同为全量 pool_tags），却是第二个独立 urltest，等于把每轮健康检查
        // 请求数从 805 抬到 1175（+46%，实测），且多出的 370 次探测全部经代理
        // 打向 gstatic，消耗用户带宽与机场配额，换来的优选结果与 auto 一模一样。
        //
        // 改后语义：balance 变为「地区优选聚合器」——成员是各 {region}-auto 组
        // 与全部节点，零探测开销（selector 不发健康检查），default 指向 auto
        // 保持「默认走全局最优」的既有用户体验；用户可在 ClashAPI/前端面板
        // 手动切到任一地区组，实现"锁定某地区自动优选"。
        // 前端 GroupSidebar 对 tag=="balance" 的图标/文案分支不受类型影响。
        let balance_members: Vec<String> = {
            let mut members: Vec<String> = vec!["auto".to_string()];
            members.extend(sorted_regions.iter().map(|r| format!("{}-auto", r)));
            members.extend(pool_tags);
            members
        };
        final_outbounds.push(json!({
            "type": "selector",
            "tag": "balance",
            "outbounds": balance_members,
            "default": "auto",
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

        // 2d-2. 自定义分组（settings.custom_group_rules，前端「自定义区域管理」维护）：
        // group_type=virtual 不生成真实组；selector/urltest/balance 生成内核真实策略组，
        // 其中 balance 当前仍按 URLTest 兼容语义生成，tag = custom-{name}。
        // 并挂入 proxy 主组成员。匹配空集不生成（sing-box 拒载空成员组）；
        // tag 与既有组/节点撞名时跳过；unlock 匹配依赖 with_unlock_state 传入的最新检测结果。
        let mut custom_group_tags: Vec<String> = Vec::new();
        for rule in &self.custom_group_rules {
            let enabled = rule
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let name = rule
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if !enabled || name.is_empty() {
                continue;
            }
            let group_type = rule
                .get("group_type")
                .and_then(|v| v.as_str())
                .unwrap_or("virtual")
                .to_string();
            if group_type == "virtual" {
                continue;
            }
            let match_type = rule
                .get("match_type")
                .and_then(|v| v.as_str())
                .unwrap_or("keyword")
                .to_string();

            // 匹配参数预提取（与前端语义对齐：keyword 包含 / regex 正则 / protocol 协议 / unlock 状态）
            let compiled = if match_type == "regex" {
                rule.get("pattern")
                    .and_then(|v| v.as_str())
                    .and_then(|p| regex::Regex::new(p).ok())
            } else {
                None
            };
            let keywords: Vec<String> = rule
                .get("keywords")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|k| k.as_str())
                        .map(|s| s.to_lowercase())
                        .collect()
                })
                .unwrap_or_default();
            let protocols: Vec<String> = rule
                .get("protocols")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|k| k.as_str())
                        .map(|s| s.to_lowercase())
                        .collect()
                })
                .unwrap_or_default();
            let unlock_service = rule
                .get("unlock")
                .and_then(|u| u.get("service"))
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let unlock_status = rule
                .get("unlock")
                .and_then(|u| u.get("status"))
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();

            let mut member_tags: Vec<String> = Vec::new();
            for out in &self.outbounds {
                let hit = match match_type.as_str() {
                    "keyword" => {
                        !keywords.is_empty()
                            && keywords
                                .iter()
                                .any(|kw| out.tag.to_lowercase().contains(kw))
                    }
                    "regex" => compiled
                        .as_ref()
                        .map(|re| re.is_match(&out.tag))
                        .unwrap_or(false),
                    "protocol" => {
                        !protocols.is_empty()
                            && protocols.iter().any(|p| out.r#type.to_lowercase() == *p)
                    }
                    "unlock" => {
                        !unlock_service.is_empty()
                            && !unlock_status.is_empty()
                            && self
                                .unlock_state
                                .get(&out.tag)
                                .and_then(|s| s.get(&unlock_service))
                                .and_then(|v| v.as_str())
                                .map(|st| st == unlock_status)
                                .unwrap_or(false)
                    }
                    _ => false,
                };
                if hit {
                    member_tags.push(out.tag.clone());
                }
            }
            if member_tags.is_empty() {
                log::warn!("[config] 自定义分组 [{}] 无匹配节点，跳过生成", name);
                continue;
            }

            let group_tag = format!("custom-{}", name);
            // tag 冲突：与既有出站组/节点 tag 撞名则跳过
            let conflict = final_outbounds
                .iter()
                .any(|o| o.get("tag").and_then(|t| t.as_str()) == Some(group_tag.as_str()))
                || all_node_tags.iter().any(|t| t == &group_tag);
            if conflict {
                log::warn!(
                    "[config] 自定义分组 tag [{}] 与既有出站冲突，跳过生成",
                    group_tag
                );
                continue;
            }

            let group_json = match group_type.as_str() {
                "selector" => json!({
                    "type": "selector", "tag": group_tag, "outbounds": member_tags,
                    "interrupt_exist_connections": false
                }),
                "urltest" => {
                    let (interval, url, tolerance) = self.urltest_params(&group_tag);
                    json!({
                        "type": "urltest", "tag": group_tag, "outbounds": member_tags,
                        "url": url, "interval": interval, "idle_timeout": "15m",
                        "tolerance": tolerance, "interrupt_exist_connections": false
                    })
                }
                "balance" => {
                    // P1 修复：与主 balance 组同理——原实现按 urltest 生成，
                    // 会重复发健康检查。此处降级为 selector（零探测开销），
                    // 成员与首选语义保持不变：第一个成员即组内首选，
                    // 用户可在面板手动改选。
                    json!({
                        "type": "selector", "tag": group_tag, "outbounds": member_tags,
                        "interrupt_exist_connections": false
                    })
                }
                _ => continue,
            };
            final_outbounds.push(group_json);
            custom_group_tags.push(group_tag);
        }
        // 自定义组挂入 proxy 主组：插在其他策略组（auto / balance / {region}-auto）之后、
        // 实体节点之前。尾部追加会让它沉在数百个成员末尾，面板里等同于看不见。
        if !custom_group_tags.is_empty() {
            let policy_tags: std::collections::HashSet<String> = final_outbounds
                .iter()
                .filter(|o| {
                    matches!(
                        o.get("type").and_then(|t| t.as_str()),
                        Some("selector") | Some("urltest")
                    )
                })
                .filter_map(|o| o.get("tag").and_then(|t| t.as_str()).map(|s| s.to_string()))
                .collect();
            for ob in final_outbounds.iter_mut() {
                if ob.get("tag").and_then(|t| t.as_str()) != Some("proxy") {
                    continue;
                }
                if let Some(arr) = ob.get_mut("outbounds").and_then(|o| o.as_array_mut()) {
                    let at = arr
                        .iter()
                        .position(|t| !policy_tags.contains(t.as_str().unwrap_or_default()))
                        .unwrap_or(arr.len());
                    arr.splice(
                        at..at,
                        custom_group_tags
                            .iter()
                            .map(|t| serde_json::Value::String(t.clone())),
                    );
                }
            }
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

        // ---- 阶段4: 组装路由 ----
        // 检查本地 rule-set 文件是否存在（以 Path::exists() 实际检查为准）
        // DNS 分流与 route 规则统一使用同一组 has_geosite/has_geoip 布尔值，
        // 避免 DNS 引用 geosite-cn 而 route 未注册该 rule-set 的不一致门控
        let has_geosite = self
            .geosite_cn_path
            .as_ref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false);
        let has_geoip = self
            .geoip_cn_path
            .as_ref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false);

        // dns.rules 与 rebuild_config_from_settings 共用 build_dns_rules（防两路径漂移）
        let is_fake_ip = self.dns_mode.trim().to_lowercase() != "realip";
        let dns_rules = build_dns_rules(
            &server_domains,
            has_geosite,
            has_geoip,
            self.dns_smart_routing_v2,
            is_fake_ip,
        );

        let mut rule_set_config = Vec::new();
        // geosite 与 geoip 各自独立注册（与 build_full_route_rules 的独立布尔语义对齐）。
        // M3-1 remote 化：type:local → remote + initial_path——
        //   - 应用层首启拉取 .srs 缓存（download_rule_set），内核启动经 initial_path
        //     直接加载本地（断网冷启动不阻塞，实测 0.01s 起）
        //   - 之后内核按 url 后台自动更新（规则集变更无应用层干预），
        //     http_client.detour=proxy：规则源（jsdelivr/GitHub raw）直连常不可达，
        //     经代理下载保证可达；缓存文件为内核回写与 initial_path 共用同一份
        //   - download_detour 为 1.14 deprecated 字段（1.16 移除，实测有警告），用 http_client 新语义
        let mk_remote_rule_set = |tag: &str, path: &str| {
            json!({
                "tag": tag,
                "type": "remote",
                "format": "binary",
                "url": if tag == "geosite-cn" {
                    "https://fastly.jsdelivr.net/gh/SagerNet/sing-geosite@rule-set/geosite-cn.srs"
                } else {
                    "https://fastly.jsdelivr.net/gh/SagerNet/sing-geoip@rule-set/geoip-cn.srs"
                },
                "initial_path": path,
                "http_client": { "detour": "proxy" }
            })
        };
        if has_geosite {
            rule_set_config.push(mk_remote_rule_set(
                "geosite-cn",
                self.geosite_cn_path.as_ref().unwrap(),
            ));
        }
        if has_geoip {
            rule_set_config.push(mk_remote_rule_set(
                "geoip-cn",
                self.geoip_cn_path.as_ref().unwrap(),
            ));
        }
        if rule_set_config.is_empty() {
            log::warn!("[config] geosite-cn.srs / geoip-cn.srs 本地文件均不存在，跳过国内直连规则，所有流量走代理");
        }

        let valid_outbound_tags: std::collections::HashSet<String> = final_outbounds
            .iter()
            .filter_map(|outbound| outbound.get("tag").and_then(|tag| tag.as_str()))
            .map(str::to_string)
            .collect();
        let route_rules =
            build_full_route_rules_filtered(has_geosite, has_geoip, Some(&valid_outbound_tags));

        let route = if rule_set_config.is_empty() {
            json!({
                "rules": route_rules,
                "final": "proxy",
                "auto_detect_interface": true,
                // P1 修复：应用级流量统计依赖 ClashAPI connections 的
                // metadata.processPath，而内核仅在 needFindProcess 为真时才做进程
                // 搜索（route/router.go: needFindProcess = hasRule(isProcessRule)
                // || options.FindProcess）。App-Matrix 为空时无 process 规则 →
                // 进程搜索器不创建 → processPath 恒为空串（实测确认），
                // traffic_monitor 会把所有连接记成 Unknown。
                // 显式开启后内核为无进程规则的连接也填充 processPath。
                // 代价：每连接一次进程查询，内核侧有 200ms LRU 缓存
                // （route/process_cache.go processCache.SetLifetime）。
                "find_process": true
            })
        } else {
            json!({
                "rule_set": rule_set_config,
                "rules": route_rules,
                "final": "proxy",
                "auto_detect_interface": true,
                // 见上方无 rule_set 分支的说明
                "find_process": true
            })
        };

        // 组装 dns.servers
        let mut dns_servers = Vec::new();
        dns_servers.push(json!({
            "detour": "proxy",
            "server": if self.dns_remote_doh.trim().is_empty() { "8.8.8.8" } else { self.dns_remote_doh.trim() },
            "tag": "remote",
            "type": "https"
        }));
        if is_fake_ip {
            // sing-box 1.14 规范的 Fake-IP DNS 服务器配置
            dns_servers.push(json!({
                "tag": "fakeip",
                "type": "fakeip",
                "inet4_range": "198.18.0.0/15",
                "inet6_range": "fc00::/18"
            }));
        }
        dns_servers.push(canonical_bootstrap_server(&self.dns_bootstrap_doh));
        dns_servers.push(canonical_bootstrap_backup_server(
            &self.dns_bootstrap_backup_doh,
        ));
        dns_servers.push(json!({
            "tag": "local",
            "type": "local",
            // 1.14.0：单标签与 .lan/.local 后缀域名走系统邻居解析器
            "neighbor_domain": [".", ".lan", ".local"]
        }));

        // ---- 阶段5: 组装最终 JSON ----
        let config = json!({
            // $schema 注入（plan-Q A.4-3）：内核忽略此字段，用户把 config.json
            // 拿到 VS Code 等兼容编辑器可自动获得字段补全与校验
            "$schema": "https://sing-box.sagernet.org/schema.json",
            "log": {
                // P1 修复：见 generate_minimal_config 同名注释（370 节点下
                // info 级逐连接日志导致 1.7MB/天 + 每行一次文件开关系统调用）。
                "level": "warn",
                "timestamp": true
            },
            "dns": {
                "servers": dns_servers,
                "rules": dns_rules,
                // sing-box 规范与内核强校验：default server 严禁为 fakeip（changelog: default server can no longer be fakeip）
                // 待代理域名由 dns.rules 尾部规则 route 到 fakeip，此处 final 保持为真实 remote DoH 承担内部兜底
                "final": "remote",
                "strategy": "prefer_ipv4",
                // P2 优化：DNS LRU 容量。内核默认取 max(cache_capacity, 1024)
                // （dns/client.go NewClient），1024 条在 370 节点 + 日常浏览
                // 场景下容易触顶重解析。4096 足以覆盖一个高频使用会话。
                "cache_capacity": 4096,
                // 1.14.0：乐观缓存（过期立即返回+后台刷新）与查询超时（快速失败）
                "optimistic": self.dns_optimistic_cache,
                "timeout": format!("{}s", self.dns_timeout_secs.max(1))
            },
            "inbounds": [
                {
                    "type": "mixed",
                    "tag": "mixed-in",
                    "listen": if self.allow_lan { "0.0.0.0" } else { "127.0.0.1" },
                    "listen_port": self.mixed_port,
                    // P2 优化：TCP Fast Open（shared/listen.md tcp_fast_open）。
                    // 代理入口是本机应用 → 内核的高频短连接路径（浏览器并发建连），
                    // TFO 可省掉一个 RTT 的握手延迟。
                    "tcp_fast_open": true
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
                    // DNS 缓存与 Fake-IP 双向映射持久化（experimental/cache-file.md）
                    // 避免内核重启/热重载后丢失映射表，导致客户端系统 DNS 缓存中未过期的 Fake-IP 寻址失效
                    "store_fakeip": true,
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

    if tag.contains("🇭🇰") {
        return "HK".to_string();
    }
    if tag.contains("🇯🇵") {
        return "JP".to_string();
    }
    if tag.contains("🇺🇸") {
        return "US".to_string();
    }
    if tag.contains("🇹🇼") {
        return "TW".to_string();
    }
    if tag.contains("🇸🇬") {
        return "SG".to_string();
    }
    if tag.contains("🇰🇷") {
        return "KR".to_string();
    }

    if lower.contains("香港") {
        return "HK".to_string();
    }
    if lower.contains("日本") {
        return "JP".to_string();
    }
    if lower.contains("美国") || lower.contains("美國") {
        return "US".to_string();
    }
    if lower.contains("台湾") || lower.contains("臺灣") || lower.contains("台灣") {
        return "TW".to_string();
    }
    if lower.contains("新加坡") {
        return "SG".to_string();
    }
    if lower.contains("韩国") || lower.contains("韓國") {
        return "KR".to_string();
    }

    let tokens = tokenize_tag(&lower);
    if tokens
        .iter()
        .any(|&t| t == "hk" || t == "hongkong" || t == "hong")
    {
        return "HK".to_string();
    }
    if tokens.iter().any(|&t| t == "jp" || t == "japan") {
        return "JP".to_string();
    }
    if tokens
        .iter()
        .any(|&t| t == "us" || t == "usa" || t == "united" || t == "america")
    {
        return "US".to_string();
    }
    if tokens.iter().any(|&t| t == "tw" || t == "taiwan") {
        return "TW".to_string();
    }
    if tokens.iter().any(|&t| t == "sg" || t == "singapore") {
        return "SG".to_string();
    }
    if tokens.iter().any(|&t| t == "kr" || t == "korea") {
        return "KR".to_string();
    }

    "OTHER".to_string()
}

fn tokenize_tag(s: &str) -> Vec<&str> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 构造 canonical 直连 DoH DNS 服务器（节点域名解析主备共用）。
/// 空/非法输入回退默认 default_addr；若填写的是域名（如 dns.alidns.com），
/// 依 docs/sing-box_docs/dns/server/https.zh.md 必须设置 domain_resolver
/// 解析该域名——用 local 承担（不依赖代理，无回环）。不设 detour：
/// 新版 https 服务器默认直连出站（防“用代理解析代理服务器地址”回环）。
/// ConfigBuilder::build 与 rebuild_config_from_settings 共用（防两路径漂移）。
fn canonical_direct_doh_server(tag: &str, addr: &str, default_addr: &str) -> Value {
    let trimmed = addr.trim();
    let addr = if trimmed.is_empty() {
        default_addr
    } else {
        trimmed
    };
    let mut obj = json!({
        "tag": tag,
        "type": "https",
        "server": addr
    });
    if addr.parse::<std::net::IpAddr>().is_err() {
        obj["domain_resolver"] = json!("local");
    }
    obj
}

/// 节点域名解析主解析器（默认 223.5.5.5 alidns）
pub fn canonical_bootstrap_server(addr: &str) -> Value {
    canonical_direct_doh_server("bootstrap", addr, "223.5.5.5")
}

/// 节点域名解析备用解析器（默认 1.12.12.12 dnspod；须与主解析器异构运营商，
/// 否则主备共享同一套递归缓存，负缓存毒化时对冲失效）
pub fn canonical_bootstrap_backup_server(addr: &str) -> Value {
    canonical_direct_doh_server("bootstrap-backup", addr, "1.12.12.12")
}

/// 构建 dns.rules 规则列表（公共函数，供 ConfigBuilder::build 与
/// rebuild_config_from_settings 统一调用——两条生成路径共用同一实现防漂移）
///
/// 规则顺序（不可变，内核按序匹配）：
/// 1. Direct 模式全量本地解析（TUN 劫持查询不依赖 detour=proxy 的 remote DoH）
/// 2. 节点服务器域名 → 主备对冲链（1.14.0 evaluate/response_rcode/respond）：
///    - evaluate 先问 bootstrap（主直连 DoH；不用 local：运营商递归不可靠；
///      不用 remote：用代理解析代理服务器地址回环）
///    - 响应为 NXDOMAIN/SERVFAIL → route 到 bootstrap-backup（异构运营商
///      独立缓存：机场子域轮换的删除窗口会被单一递归器按 SOA 负缓存放大成
///      约 10 分钟死区——2026-09-18 TUN 全瘫事故根因——备用递归器大概率
///      未踩中该窗口，一次查询内立即愈合）
///    - 其余响应 → respond 直接采用主解析器答案（正常路径零额外查询）
///    - 兜底 route → bootstrap：evaluate 无响应（网络抖动）时查询不落空到
///      final=remote 造成回环
/// 3. 智能分流 v2（smart_v2 && has_geoip 时启用，1.14.0 响应级分流）：
///    - evaluate 向 local 发查询并保存响应（不终止匹配）
///    - 答案 IP 命中 geoip-cn → respond 直接采用本地答案（后续 route 按 IP 直连）
///    - 否则 fallthrough 到 geosite-cn 名单规则 / remote DoH
///    （v2 链整块注入替换 geosite-cn 名单规则：名单语义被解析结果归属取代）
/// 4. geosite-cn 名单直连（v1 语义，仅 smart_v2 未启用时注入）
///
/// has_geoip=false（无订阅态/rule-set 未就绪）时 v2 链不注入，
/// match_response 引用未注册 rule-set 会在内核初始化期拒载。
pub fn build_dns_rules(
    server_domains: &[String],
    has_geosite: bool,
    has_geoip: bool,
    smart_v2: bool,
    is_fake_ip: bool,
) -> Vec<Value> {
    let mut rules = vec![
        // Direct 模式全量本地解析（与 route 的 clash_mode 单一真相源对齐）：
        // TUN 下系统 DNS 劫持的查询在 Direct 模式走 local，不依赖 remote DoH / fakeip
        //（detour "proxy" 引用不经过 route.rules，Direct 前置规则对 DoH 无效，
        //  节点故障时直连模式域名解析不应随之失败）
        json!({ "clash_mode": "Direct", "action": "route", "server": "local" }),
    ];

    if !server_domains.is_empty() {
        // 节点域名主备对冲链（带 disable_cache: true 防本地固化负缓存死区）：
        // 1. evaluate 先查主直连 DoH (bootstrap，默认 223.5.5.5)；
        // 2. 响应为 NXDOMAIN/SERVFAIL 时切备用直连 DoH (bootstrap-backup，默认 1.12.12.12)；
        // 3. respond 采纳答案；
        // 4. 兜底 route -> bootstrap，保证链内必定闭合，绝不泄漏至 proxy 回环！
        rules.push(json!({ "domain": server_domains, "action": "evaluate", "server": "bootstrap", "disable_cache": true }));
        rules.push(json!({ "domain": server_domains, "match_response": true, "response_rcode": "NXDOMAIN", "action": "route", "server": "bootstrap-backup", "disable_cache": true }));
        rules.push(json!({ "domain": server_domains, "match_response": true, "response_rcode": "SERVFAIL", "action": "route", "server": "bootstrap-backup", "disable_cache": true }));
        rules
            .push(json!({ "domain": server_domains, "match_response": true, "action": "respond" }));
        rules.push(json!({ "domain": server_domains, "action": "route", "server": "bootstrap", "disable_cache": true }));

        // 提取公共主根域（如 9999231.xyz）通配直连兜底：
        // 防止机场后端高频动态轮换出的新子域尚未进入订阅快照时，因未匹配而漏入 proxy 造成死锁回环
        let mut parent_suffixes: Vec<String> = Vec::new();
        for d in server_domains {
            let parts: Vec<&str> = d.split('.').collect();
            if parts.len() >= 2 {
                let suffix = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
                if !parent_suffixes.contains(&suffix) {
                    parent_suffixes.push(suffix);
                }
            }
        }
        if !parent_suffixes.is_empty() {
            rules.push(json!({
                "domain_suffix": parent_suffixes,
                "action": "route",
                "server": "bootstrap",
                "disable_cache": true
            }));
        }
    }

    if smart_v2 && has_geoip {
        // v2 链（顺序由生成器固定，respond 必须有先行 evaluate，内核否则运行时报错）
        rules.push(json!({ "action": "evaluate", "server": "local" }));
        rules
            .push(json!({ "match_response": true, "rule_set": ["geoip-cn"], "action": "respond" }));
        rules.push(
            json!({ "action": "route", "server": if is_fake_ip { "fakeip" } else { "remote" } }),
        );
    } else if has_geosite {
        // v1 名单语义：国内域名由 local DNS 权威解析
        rules.push(json!({ "rule_set": "geosite-cn", "action": "route", "server": "local" }));
    }

    // 境外与待代理域名分流：
    // 若开启 Fake-IP，未命中直连的域名统一分配 fakeip（0ms 响应、彻底免疫本地 GFW 投毒与公网 DNS 限流）
    // 若为 realip，则未命中规则自然落入 dns.final = remote
    if is_fake_ip {
        rules.push(json!({ "action": "route", "server": "fakeip" }));
    }

    rules
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
    build_full_route_rules_filtered(has_geosite, has_geoip, None)
}

/// 构建路由规则，并可按当前配置中的 outbound tag 过滤失效引用。
/// `valid_tags` 为 None 时仅执行不依赖当前配置的 default 清理，供纯函数测试/兼容调用使用。
pub fn build_full_route_rules_filtered(
    has_geosite: bool,
    has_geoip: bool,
    valid_tags: Option<&std::collections::HashSet<String>>,
) -> Vec<Value> {
    let mut rules = vec![
        // P2 优化：限定嗅探协议范围。默认 `{"action":"sniff"}` 会启用全部
        // 嗅探器（route/sniff.md 共 11 种，含 bittorrent/rdp/ssh/dtls/ntp/stun），
        // 对本项目的分流需求（域名识别 + QUIC 识别）没有额外贡献，却是每连接
        // 的固定 CPU 开销。只保留实际会用到 的四种。
        json!({ "action": "sniff", "sniffer": ["http", "tls", "quic", "dns"] }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
    ];
    let has_tag = |tag: &str| valid_tags.map_or(true, |tags| tags.contains(tag));
    // Direct 模式全量直连（clash_mode 单一真相源：运行时 PATCH mode 与
    // 重启后 default_mode 行为一致；此规则须在所有分流规则之前）
    if has_tag("direct") {
        rules.push(json!({ "clash_mode": "direct", "action": "route", "outbound": "direct" }));
    }
    // Global 模式显式绑定到 proxy；该规则同时让 ClashAPI 的 mode-list
    // 在存在 proxy 时包含 global，而不是依赖不可见的隐式模式集合。
    // 放在用户显式规则之后，保持显式 App/Custom 规则优先级。
    // 1. 注入 App-Matrix 应用分流规则 (优先级高于通用域名分流)
    let app_rules = crate::commands::routing::load_app_rules_internal();
    let mut app_rules: Vec<_> = app_rules.into_iter().collect();
    app_rules.sort_by(|a, b| a.0.cmp(&b.0));
    for (proc_name, outbound) in app_rules {
        let outbound = outbound.trim();
        if proc_name.trim().is_empty()
            || outbound.is_empty()
            || outbound == "default"
            || valid_tags.is_some_and(|tags| !tags.contains(outbound))
        {
            continue;
        }
        rules.push(json!({
            "process_name": [proc_name],
            "outbound": outbound
        }));
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
        if outbound == "default" || valid_tags.is_some_and(|tags| !tags.contains(&outbound)) {
            continue;
        }

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
    if has_geosite && has_tag("direct") {
        rules.push(json!({
            "clash_mode": "rule",
            "rule_set": ["geosite-cn"],
            "outbound": "direct"
        }));
    }

    // 4. 私有内网 IP 直连规则 (放置在域名规则之后)
    if has_tag("direct") {
        rules.push(json!({
            "ip_is_private": true,
            "outbound": "direct"
        }));
    }

    // 5. 注入自定义 IP CIDR 规则
    rules.extend(custom_ip_rules);

    // 6. 注入国内 IP 规则集直连规则 (geoip-cn)
    if has_geoip && has_tag("direct") {
        rules.push(json!({
            "clash_mode": "rule",
            "rule_set": ["geoip-cn"],
            "outbound": "direct"
        }));
    }

    // Global 兜底规则仅在 proxy 真实存在时生成；放在显式规则之后，
    // 既让 mode-list 暴露 Global，又不覆盖用户显式 App/Custom 分流。
    if has_tag("proxy") {
        rules.push(json!({
            "clash_mode": "global",
            "action": "route",
            "outbound": "proxy"
        }));
    }

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- build_dns_rules（智能分流 v2 共享函数）四象限 ----

    #[test]
    fn test_build_dns_rules_v2_enabled_with_geoip() {
        // v2 开 + geoip 就绪 + realip 模式：节点域名主备对冲链 + evaluate/match_response/respond 三行链
        let rules = build_dns_rules(&["node.example.com".to_string()], true, true, true, false);
        assert_eq!(
            rules[0],
            json!({ "clash_mode": "Direct", "action": "route", "server": "local" }),
            "Direct 前置必须保持首条"
        );
        // 节点域名对冲链：evaluate bootstrap → NXDOMAIN/SERVFAIL 切 backup → respond → 兜底 route
        assert_eq!(
            rules[1],
            json!({ "domain": ["node.example.com"], "action": "evaluate", "server": "bootstrap", "disable_cache": true })
        );
        assert_eq!(
            rules[2],
            json!({ "domain": ["node.example.com"], "match_response": true, "response_rcode": "NXDOMAIN", "action": "route", "server": "bootstrap-backup", "disable_cache": true })
        );
        assert_eq!(
            rules[3],
            json!({ "domain": ["node.example.com"], "match_response": true, "response_rcode": "SERVFAIL", "action": "route", "server": "bootstrap-backup", "disable_cache": true })
        );
        assert_eq!(
            rules[4],
            json!({ "domain": ["node.example.com"], "match_response": true, "action": "respond" })
        );
        assert_eq!(
            rules[5],
            json!({ "domain": ["node.example.com"], "action": "route", "server": "bootstrap", "disable_cache": true })
        );
        // 通配根域直连兜底
        assert_eq!(
            rules[6],
            json!({ "domain_suffix": ["example.com"], "action": "route", "server": "bootstrap", "disable_cache": true })
        );
        // v2 智能分流
        assert_eq!(
            rules[7],
            json!({ "action": "evaluate", "server": "local" }),
            "evaluate 必须先于 respond"
        );
        assert_eq!(
            rules[8],
            json!({ "match_response": true, "rule_set": ["geoip-cn"], "action": "respond" })
        );
        assert_eq!(rules[9], json!({ "action": "route", "server": "remote" }));
        assert_eq!(rules.len(), 10);
    }

    #[test]
    fn test_build_dns_rules_fakeip_mode() {
        // Fake-IP 模式测试：未命中直连的待代理域名应 route 到 fakeip
        let rules = build_dns_rules(&["node.example.com".to_string()], true, true, true, true);
        let fakeip_rule = rules.iter().find(|r| {
            r.get("action").and_then(|a| a.as_str()) == Some("route")
                && r.get("server").and_then(|s| s.as_str()) == Some("fakeip")
                && r.get("domain").is_none()
        });
        assert!(
            fakeip_rule.is_some(),
            "Fake-IP 模式必须生成 route 到 fakeip 的分流规则"
        );
    }

    #[test]
    fn test_node_domain_hedge_chain_terminates_before_v2() {
        // 结构性约束：节点域名链必须自带无条件兜底 route（链内终止），
        // 否则 evaluate 无响应时查询会穿透到 v2 链（evaluate→local，污染语义）
        // 或 final=remote（用代理解析代理服务器地址，回环）
        let rules = build_dns_rules(&["node.example.com".to_string()], true, true, true, false);
        let chain_end = rules
            .iter()
            .position(|r| {
                r.get("domain").is_some()
                    && r.get("action").and_then(|a| a.as_str()) == Some("route")
                    && r.get("server").and_then(|s| s.as_str()) == Some("bootstrap")
                    && r.get("match_response").is_none()
            })
            .expect("对冲链必须有无条件兜底 route→bootstrap");
        let v2_start = rules
            .iter()
            .position(|r| {
                r.get("action").and_then(|a| a.as_str()) == Some("evaluate")
                    && r.get("server").and_then(|s| s.as_str()) == Some("local")
            })
            .expect("v2 链应存在");
        assert!(chain_end < v2_start, "对冲链必须在 v2 链之前闭合");
    }

    #[test]
    fn test_build_dns_rules_v2_disabled_falls_back_to_geosite() {
        // v2 关 + geosite 就绪：恢复 v1 名单语义（回归保障）
        let rules = build_dns_rules(&[], true, true, false, false);
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules[1],
            json!({ "rule_set": "geosite-cn", "action": "route", "server": "local" })
        );
        assert!(!rules
            .iter()
            .any(|r| r.get("action").and_then(|a| a.as_str()) == Some("evaluate")));
        assert!(!rules
            .iter()
            .any(|r| r.get("match_response").and_then(|m| m.as_bool()) == Some(true)));
    }

    #[test]
    fn test_build_dns_rules_v2_enabled_without_geoip_not_injected() {
        // v2 开但 geoip 未就绪（无订阅态）：不注入 v2 链（match_response 引用
        // 未注册 rule-set 会拒载），geosite 可用时回退 v1 语义
        let rules = build_dns_rules(&[], true, false, true, false);
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules[1],
            json!({ "rule_set": "geosite-cn", "action": "route", "server": "local" })
        );
        assert!(
            !rules.iter().any(|r| r.get("match_response").is_some()),
            "geoip 缺失时不得注入 v2 链"
        );
    }

    #[test]
    fn test_build_dns_rules_no_rule_sets_minimal() {
        // 双 rule-set 均缺失 + v2 开：仅剩 Direct 前置（无任何名单/响应链）
        let rules = build_dns_rules(&[], false, false, true, false);
        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0],
            json!({ "clash_mode": "Direct", "action": "route", "server": "local" })
        );
    }

    #[test]
    fn test_build_dns_rules_empty_domains_skip_domain_rule() {
        // 节点 server 全为 IP 时域名规则不注入
        let rules = build_dns_rules(&[], false, false, false, false);
        assert!(!rules.iter().any(|r| r.get("domain").is_some()));
    }

    /// 用临时 .srs 文件让 has_geoip/has_geosite 为真，验证 ConfigBuilder
    /// 全量生成路径注入 v2 链（与单测纯函数互补，覆盖 build 集成）
    #[test]
    fn test_config_builder_injects_v2_chain() {
        let tmp = std::env::temp_dir();
        let geosite = tmp.join("auroweave-test-geosite-cn.srs");
        let geoip = tmp.join("auroweave-test-geoip-cn.srs");
        std::fs::write(&geosite, b"stub").unwrap();
        std::fs::write(&geoip, b"stub").unwrap();

        let outbounds = vec![make_node("🇯🇵 日本-001")];
        let builder = ConfigBuilder::new(outbounds)
            .with_local_rule_sets(
                Some(geosite.to_string_lossy().to_string()),
                Some(geoip.to_string_lossy().to_string()),
            )
            .with_dns("1.1.1.1".to_string(), 5, true, true);
        let config = builder.build().expect("build config 应该成功");

        let dns_rules = config["dns"]["rules"].as_array().unwrap();
        let evaluate_idx = dns_rules
            .iter()
            .position(|r| r.get("action").and_then(|a| a.as_str()) == Some("evaluate"));
        let respond_idx = dns_rules.iter().position(|r| {
            r.get("action").and_then(|a| a.as_str()) == Some("respond")
                && r.get("match_response").and_then(|m| m.as_bool()) == Some(true)
        });
        let fakeip_idx = dns_rules.iter().position(|r| {
            r.get("action").and_then(|a| a.as_str()) == Some("route")
                && r.get("server").and_then(|s| s.as_str()) == Some("fakeip")
        });
        assert!(evaluate_idx.is_some(), "v2 开启时 evaluate 应存在");
        assert!(
            respond_idx.is_some(),
            "v2 开启时 match_response+respond 应存在"
        );
        assert!(
            fakeip_idx.is_some(),
            "fakeip 模式下 v2 链尾 route→fakeip 应存在"
        );
        assert!(
            evaluate_idx.unwrap() < respond_idx.unwrap(),
            "evaluate 必须先于 respond"
        );
        assert!(
            respond_idx.unwrap() < fakeip_idx.unwrap(),
            "respond 必须先于兜底 route→fakeip"
        );

        // route.rule_set 必须注册 geoip-cn（v2 链引用它）
        let route_rule_sets = config["route"]["rule_set"].as_array().unwrap();
        assert!(route_rule_sets
            .iter()
            .any(|rs| rs.get("tag").and_then(|t| t.as_str()) == Some("geoip-cn")));

        // realip 模式下验证尾部 route 到 remote
        let builder_realip = ConfigBuilder::new(vec![make_node("🇯🇵 日本-001")])
            .with_dns_mode("realip")
            .with_local_rule_sets(
                Some(geosite.to_string_lossy().to_string()),
                Some(geoip.to_string_lossy().to_string()),
            )
            .with_dns("1.1.1.1".to_string(), 5, true, true);
        let config_realip = builder_realip
            .build()
            .expect("build config realip 应该成功");
        let dns_rules_realip = config_realip["dns"]["rules"].as_array().unwrap();
        let remote_idx = dns_rules_realip.iter().position(|r| {
            r.get("action").and_then(|a| a.as_str()) == Some("route")
                && r.get("server").and_then(|s| s.as_str()) == Some("remote")
        });
        assert!(
            remote_idx.is_some(),
            "realip 模式下 v2 链尾 route→remote 应存在"
        );

        let _ = std::fs::remove_file(&geosite);
        let _ = std::fs::remove_file(&geoip);
    }

    #[test]
    fn test_build_full_route_rules_order() {
        let rules = build_full_route_rules(true, true);
        assert!(!rules.is_empty());

        // 验证域名规则 (geosite-cn) 在私有 IP 规则 (ip_is_private) 之前
        let geosite_idx = rules.iter().position(|r| {
            r.get("rule_set")
                .and_then(|rs| rs.as_array())
                .map(|arr| arr.iter().any(|item| item.as_str() == Some("geosite-cn")))
                .unwrap_or(false)
        });
        let ip_private_idx = rules.iter().position(|r| {
            r.get("ip_is_private")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        });

        assert!(geosite_idx.is_some());
        assert!(ip_private_idx.is_some());
        assert!(
            geosite_idx.unwrap() < ip_private_idx.unwrap(),
            "geosite-cn 规则必须排在 ip_is_private 规则之前"
        );
    }

    #[test]
    fn test_custom_group_real_outbounds() {
        let outbounds = vec![make_node("🇯🇵 日本-极速-001"), make_node("🇭🇰 香港-极速-001")];
        let rules = vec![
            json!({
                "name": "日本优选", "enabled": true, "group_type": "selector",
                "match_type": "keyword", "keywords": ["日本", "JP"], "order": 0
            }),
            json!({
                "name": "空匹配组", "enabled": true, "group_type": "urltest",
                "match_type": "keyword", "keywords": ["不存在的关键词"], "order": 1
            }),
            json!({
                "name": "纯展示组", "enabled": true, "group_type": "virtual",
                "match_type": "keyword", "keywords": ["香港"], "order": 2
            }),
        ];
        let config = ConfigBuilder::new(outbounds)
            .with_custom_groups(rules)
            .build()
            .expect("build config 应该成功");

        let outs = config["outbounds"].as_array().unwrap();
        let jp = outs
            .iter()
            .find(|o| o.get("tag").and_then(|t| t.as_str()) == Some("custom-日本优选"))
            .expect("应生成 selector 自定义组");
        assert_eq!(jp.get("type").and_then(|t| t.as_str()), Some("selector"));
        let members = jp.get("outbounds").and_then(|o| o.as_array()).unwrap();
        assert!(members
            .iter()
            .any(|m| m.as_str() == Some("🇯🇵 日本-极速-001")));
        assert!(
            !members
                .iter()
                .any(|m| m.as_str() == Some("🇭🇰 香港-极速-001")),
            "关键词不匹配的节点不应入组"
        );

        assert!(
            outs.iter()
                .all(|o| o.get("tag").and_then(|t| t.as_str()) != Some("custom-空匹配组")),
            "空匹配组不应生成"
        );
        assert!(
            outs.iter()
                .all(|o| o.get("tag").and_then(|t| t.as_str()) != Some("custom-纯展示组")),
            "virtual 类型不进内核"
        );

        // proxy 主组应挂入自定义组 tag
        let proxy = outs
            .iter()
            .find(|o| o.get("tag").and_then(|t| t.as_str()) == Some("proxy"))
            .unwrap();
        let proxy_members = proxy.get("outbounds").and_then(|o| o.as_array()).unwrap();
        assert!(
            proxy_members
                .iter()
                .any(|m| m.as_str() == Some("custom-日本优选")),
            "自定义组应挂入 proxy 主组"
        );

        // 自定义组须排在实体节点之前（与其他策略组同区），否则在数百成员的
        // proxy 列表里沉底看不见
        let custom_idx = proxy_members
            .iter()
            .position(|m| m.as_str() == Some("custom-日本优选"))
            .unwrap();
        let node_idx = proxy_members
            .iter()
            .position(|m| m.as_str() == Some("🇯🇵 日本-极速-001"))
            .unwrap();
        assert!(custom_idx < node_idx, "自定义组应排在实体节点之前");
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
        let servers = dns
            .get("servers")
            .and_then(|s| s.as_array())
            .expect("servers 应该存在");
        let local_srv = servers
            .iter()
            .find(|s| s.get("tag").and_then(|t| t.as_str()) == Some("local"))
            .expect("应该包含 local dns");
        assert_eq!(
            local_srv.get("type").and_then(|t| t.as_str()),
            Some("local")
        );
        assert!(
            local_srv.get("server").is_none(),
            "type: local 不应有 server 字段"
        );
        // 1.14.0 neighbor_domain：局域网单标签/.lan/.local 域名走系统邻居解析器
        assert_eq!(
            local_srv
                .get("neighbor_domain")
                .and_then(|n| n.as_array())
                .map(|a| a.len()),
            Some(3),
            "local dns 应包含 neighbor_domain 3 项"
        );

        // 节点域名专用解析器：bootstrap（国内直连 DoH，不经运营商递归、不经代理防回环）
        let bootstrap_srv = servers
            .iter()
            .find(|s| s.get("tag").and_then(|t| t.as_str()) == Some("bootstrap"))
            .expect("应该包含 bootstrap dns");
        assert_eq!(
            bootstrap_srv.get("type").and_then(|t| t.as_str()),
            Some("https")
        );
        assert_eq!(
            bootstrap_srv.get("server").and_then(|s| s.as_str()),
            Some("223.5.5.5")
        );
        assert!(
            bootstrap_srv.get("detour").is_none(),
            "bootstrap 不得 detour 经代理（用代理解析代理地址会回环）"
        );

        // 备用解析器：bootstrap-backup（默认 dnspod 1.12.12.12，异构运营商独立缓存对冲）
        let backup_srv = servers
            .iter()
            .find(|s| s.get("tag").and_then(|t| t.as_str()) == Some("bootstrap-backup"))
            .expect("应该包含 bootstrap-backup dns");
        assert_eq!(
            backup_srv.get("type").and_then(|t| t.as_str()),
            Some("https")
        );
        assert_eq!(
            backup_srv.get("server").and_then(|s| s.as_str()),
            Some("1.12.12.12")
        );
        assert!(
            backup_srv.get("detour").is_none(),
            "bootstrap-backup 不得 detour 经代理"
        );

        // Direct 模式 DNS 规则必须前置（TUN 下劫持的查询在 Direct 模式走 local，
        // 不依赖 detour=proxy 的 remote DoH——节点故障时直连解析不受影响）
        let dns_rules = dns
            .get("rules")
            .and_then(|r| r.as_array())
            .expect("dns rules 应存在");
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

        // 节点服务器域名规则：主备对冲链（evaluate→bootstrap / NXDOMAIN 切 backup / respond / 兜底）
        let domain_rules: Vec<&Value> = dns_rules
            .iter()
            .filter(|r| r.get("domain").is_some())
            .collect();
        assert_eq!(domain_rules.len(), 5, "节点域名应生成 5 条对冲链规则");
        assert_eq!(
            domain_rules[0].get("action").and_then(|a| a.as_str()),
            Some("evaluate")
        );
        assert_eq!(
            domain_rules[0].get("server").and_then(|s| s.as_str()),
            Some("bootstrap")
        );
        assert_eq!(
            domain_rules[1]
                .get("response_rcode")
                .and_then(|s| s.as_str()),
            Some("NXDOMAIN"),
            "主解析器负缓存毒化时切备用"
        );
        assert_eq!(
            domain_rules[1].get("server").and_then(|s| s.as_str()),
            Some("bootstrap-backup")
        );
        assert_eq!(
            domain_rules[3].get("action").and_then(|a| a.as_str()),
            Some("respond"),
            "正常响应由 respond 采用主解析器答案"
        );
        assert_eq!(
            domain_rules[4].get("server").and_then(|s| s.as_str()),
            Some("bootstrap"),
            "兜底 route 必须在链尾防穿透"
        );

        // 验证 auto urltest 策略组中过滤了公告伪节点
        let outbounds_arr = config
            .get("outbounds")
            .and_then(|o| o.as_array())
            .expect("outbounds 应该存在");
        let auto_group = outbounds_arr
            .iter()
            .find(|o| o.get("tag").and_then(|t| t.as_str()) == Some("auto"))
            .expect("应该包含 auto 分组");
        let auto_nodes = auto_group
            .get("outbounds")
            .and_then(|n| n.as_array())
            .expect("auto 节点列表");
        let auto_tags: Vec<String> = auto_nodes
            .iter()
            .filter_map(|s| s.as_str().map(|v| v.to_string()))
            .collect();

        assert!(
            !auto_tags.contains(&"认准官网地址".to_string()),
            "auto 分组不应包含公告节点"
        );
        assert!(
            auto_tags.contains(&"🇯🇵 日本-极速-001".to_string()),
            "auto 分组应包含有效日本节点"
        );
        assert!(
            auto_tags.contains(&"🇭🇰 香港-极速-001".to_string()),
            "auto 分组应包含有效香港节点"
        );
    }

    #[test]
    fn test_config_builder_sing_box_1_14_check() {
        let outbounds = vec![make_node("🇯🇵 日本-极速-001"), make_node("🇭🇰 香港-极速-001")];
        // 1. Fake-IP 模式全量配置
        let config_fakeip = ConfigBuilder::new(outbounds.clone())
            .build()
            .expect("Fake-IP 配置构建应该成功");

        let tmp = std::env::temp_dir();
        let path_fakeip = tmp.join("test-singbox-fakeip-check.json");
        std::fs::write(
            &path_fakeip,
            serde_json::to_string_pretty(&config_fakeip).unwrap(),
        )
        .unwrap();

        // 2. Real-IP 模式全量配置
        let config_realip = ConfigBuilder::new(outbounds)
            .with_dns_mode("realip")
            .build()
            .expect("Real-IP 配置构建应该成功");
        let path_realip = tmp.join("test-singbox-realip-check.json");
        std::fs::write(
            &path_realip,
            serde_json::to_string_pretty(&config_realip).unwrap(),
        )
        .unwrap();

        // 查找项目内置 sidecar 二进制进行内核级合法性校验。
        // 不硬编码版本号（此前写死 sing-box-1.14.0，内核每次升级都要改测试代码；
        // 一旦漏改，`if exists()` 会静默跳过校验，最有价值的回归防线形同虚设）。
        // 复用 core_paths::resolve_core_binary（按 mtime 取最新的内核本体）——
        // 与运行时 SidecarManager 走同一解析逻辑，测试校验的就是实际会跑的那个二进制。
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // resolve_core_binary 的候选目录含相对路径（相对 CWD），测试运行时 CWD
        // 不保证是仓库根，故先切到 manifest_dir 再解析，随后还原。
        let prev_cwd = std::env::current_dir().ok();
        std::env::set_current_dir(&manifest_dir).ok();
        let sidecar_bin = crate::core_paths::resolve_core_binary();
        if let Some(prev) = prev_cwd {
            let _ = std::env::set_current_dir(prev);
        }

        // 找不到内核时给出显式提示而非静默跳过——静默跳过会让这道
        // 最关键的回归防线（内核级配置校验）在 CI/新环境里形同虚设。
        let Some(sidecar_bin) = sidecar_bin else {
            panic!(
                "未找到 sing-box 内核二进制（已查找 {}）；\
                 请先执行 `npm run download:sing-box` 后再跑测试",
                manifest_dir.join("sidecar-bin").display()
            );
        };

        let out_fakeip = std::process::Command::new(&sidecar_bin)
            .args(["check", "-c", &path_fakeip.to_string_lossy()])
            .output()
            .expect("执行 sing-box check fakeip 应该成功");
        let stderr_fakeip = String::from_utf8_lossy(&out_fakeip.stderr);
        assert!(
            out_fakeip.status.success(),
            "sing-box check fakeip 必须成功（内核 {}），stderr: {}",
            sidecar_bin.display(),
            stderr_fakeip
        );

        let out_realip = std::process::Command::new(&sidecar_bin)
            .args(["check", "-c", &path_realip.to_string_lossy()])
            .output()
            .expect("执行 sing-box check realip 应该成功");
        let stderr_realip = String::from_utf8_lossy(&out_realip.stderr);
        assert!(
            out_realip.status.success(),
            "sing-box check realip 必须成功（内核 {}），stderr: {}",
            sidecar_bin.display(),
            stderr_realip
        );

        let _ = std::fs::remove_file(path_fakeip);
        let _ = std::fs::remove_file(path_realip);
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
        assert_eq!(
            sorted.len(),
            outbound_tags.len(),
            "outbound tag 必须全局唯一"
        );

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
        assert!(
            auto_refs.contains(&"🇯🇵 日本-A-2".to_string()),
            "urltest 引用需同步重命名后的 tag"
        );
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
        assert!(
            hk_auto_count <= 1,
            "HK-auto 若存在只能是 urltest 分组，不能有同名节点出站"
        );
        // 真实节点保留
        assert!(outbound_tags.contains(&"🇭🇰 香港-真实-001"));
    }

    #[test]
    fn test_dns_route_gating_consistent() {
        // geosite_cn_path 指向不存在的文件时：DNS 规则与 route rule-set 均不引用 geosite-cn
        let outbounds = vec![make_node("🇯🇵 日本-001")];
        let builder = ConfigBuilder::new(outbounds).with_local_rule_sets(
            Some("/nonexistent/geosite-cn.srs".to_string()),
            Some("/nonexistent/geoip-cn.srs".to_string()),
        );
        let config = builder.build().expect("build config 应该成功");

        let dns_rules = config["dns"]["rules"].as_array().unwrap();
        let dns_refs_geosite = dns_rules.iter().any(|r| {
            r.get("rule_set")
                .and_then(|rs| rs.as_array())
                .map(|arr| arr.iter().any(|i| i.as_str() == Some("geosite-cn")))
                .unwrap_or(false)
                || r.get("rule_set").and_then(|rs| rs.as_str()) == Some("geosite-cn")
        });
        let route_rule_sets: Vec<&str> = config["route"]
            .get("rule_set")
            .and_then(|rs| rs.as_array())
            .map(|arr| arr.iter().filter_map(|i| i.as_str()).collect())
            .unwrap_or_default();

        assert!(!dns_refs_geosite, "文件不存在时 DNS 不得引用 geosite-cn");
        assert!(
            !route_rule_sets.contains(&"geosite-cn"),
            "文件不存在时 route 不得注册 geosite-cn"
        );
    }

    #[test]
    fn test_route_rules_filter_invalid_outbound_and_expose_global() {
        let tags = ["direct", "proxy"]
            .into_iter()
            .map(str::to_string)
            .collect::<std::collections::HashSet<_>>();
        let rules = build_full_route_rules_filtered(true, true, Some(&tags));
        assert!(rules.iter().any(|rule| {
            rule.get("clash_mode").and_then(|mode| mode.as_str()) == Some("global")
                && rule.get("outbound").and_then(|outbound| outbound.as_str()) == Some("proxy")
        }));
        assert!(rules.iter().all(|rule| {
            rule.get("outbound").and_then(|outbound| outbound.as_str()) != Some("default")
        }));

        let no_direct = ["proxy"]
            .into_iter()
            .map(str::to_string)
            .collect::<std::collections::HashSet<_>>();
        let rules = build_full_route_rules_filtered(true, true, Some(&no_direct));
        assert!(!rules.iter().any(|rule| {
            rule.get("outbound").and_then(|outbound| outbound.as_str()) == Some("direct")
        }));
    }

    #[test]
    fn test_custom_group_urltest_uses_group_tag_for_parameters() {
        let mut group_configs = std::collections::HashMap::new();
        group_configs.insert(
            "custom-jp".to_string(),
            crate::commands::settings::GroupTestConfig {
                interval: Some(60),
                url: Some("https://example.com/jp".to_string()),
                tolerance: Some(7),
            },
        );
        group_configs.insert(
            "custom-balance".to_string(),
            crate::commands::settings::GroupTestConfig {
                interval: Some(70),
                url: Some("https://example.com/balance".to_string()),
                tolerance: Some(9),
            },
        );
        let config = ConfigBuilder::new(vec![make_node("日本节点")])
            .with_custom_groups(vec![
                json!({
                    "name": "jp", "enabled": true, "group_type": "urltest",
                    "match_type": "keyword", "keywords": ["日本"]
                }),
                json!({
                    "name": "balance", "enabled": true, "group_type": "balance",
                    "match_type": "keyword", "keywords": ["日本"]
                }),
            ])
            .with_group_configs(group_configs)
            .build()
            .expect("build config 应该成功");
        let outbounds = config["outbounds"].as_array().unwrap();
        let jp = outbounds.iter().find(|o| o["tag"] == "custom-jp").unwrap();
        let balance = outbounds
            .iter()
            .find(|o| o["tag"] == "custom-balance")
            .unwrap();
        assert_eq!(jp["url"], "https://example.com/jp");
        assert_eq!(jp["interval"], "60s");
        assert_eq!(jp["tolerance"], 7);
        // P1 修复（2026-09-27）：balance 型自定义组不再生成 urltest。
        // 原实现按 urltest 发出与同成员 urltest 组重复的健康检查请求，
        // 现降级为 selector（零探测开销），因此不再有 url/interval/tolerance。
        // 该组仍保留全部成员，可由用户在面板手动改选。
        assert_eq!(balance["type"], "selector");
        assert!(balance.get("url").is_none());
        assert!(balance.get("interval").is_none());
        assert!(balance.get("tolerance").is_none());
        assert_eq!(balance["outbounds"], json!(["日本节点"]));
    }

    /// P0 回归（2026-09-27）：节点出站必须自带 domain_resolver，
    /// 且 route 上不得再有 default_domain_resolver——
    /// 后者会让出站解析绕过 dns.rules（详见 build() 阶段1 注释）。
    #[test]
    fn test_node_outbounds_bind_bootstrap_resolver() {
        let config = ConfigBuilder::new(vec![make_node("🇯🇵 日本-01")])
            .build()
            .expect("build config 应该成功");

        assert!(
            config["route"].get("default_domain_resolver").is_none(),
            "route 不得保留 default_domain_resolver（会使出站解析绕过 dns.rules）"
        );

        let outbounds = config["outbounds"].as_array().unwrap();
        let node = outbounds
            .iter()
            .find(|o| o["tag"] == "🇯🇵 日本-01")
            .expect("应包含节点出站");
        assert_eq!(
            node["domain_resolver"], "bootstrap",
            "节点出站必须显式绑定 bootstrap（直连 DoH），否则 1.14 拒载"
        );

        let direct = outbounds
            .iter()
            .find(|o| o["tag"] == "direct")
            .expect("应包含 direct 出站");
        assert_eq!(
            direct["domain_resolver"], "local",
            "direct 同样必须自带解析器"
        );
    }

    /// P1 回归（2026-09-27）：balance 组改为 selector，
    /// 每轮 urltest 探测请求数应从「auto + balance 全量重复」降为单份。
    #[test]
    fn test_balance_group_is_selector_without_probing() {
        let config = ConfigBuilder::new(vec![
            make_node("🇯🇵 日本-01"),
            make_node("🇺🇸 美国-01"),
        ])
        .build()
        .expect("build config 应该成功");

        let outbounds = config["outbounds"].as_array().unwrap();
        let balance = outbounds
            .iter()
            .find(|o| o["tag"] == "balance")
            .expect("应包含 balance 组");

        assert_eq!(
            balance["type"], "selector",
            "balance 不应再是 urltest（会与 auto 重复发探测）"
        );
        assert_eq!(balance["default"], "auto", "默认仍走全局最优");
        // selector 不发健康检查，故不应带 urltest 专属字段
        for field in ["url", "interval", "tolerance", "idle_timeout"] {
            assert!(
                balance.get(field).is_none(),
                "selector 组不应携带 urltest 字段 {field}"
            );
        }

        // 探测总量断言：urltest 组数应只剩 auto + 各地区组，
        // 不再出现与 auto 成员完全相同的第二个全量组。
        let auto_members: Vec<&str> = outbounds
            .iter()
            .find(|o| o["tag"] == "auto")
            .and_then(|o| o["outbounds"].as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .expect("auto 组应存在");
        let balance_members: Vec<&str> = balance["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_ne!(
            balance_members, auto_members,
            "balance 成员不应与 auto 完全相同（那正是重复探测的根因）"
        );
    }

    /// P1 回归（2026-09-27）：应用级流量统计依赖 ClashAPI processPath，
    /// 而内核仅在 needFindProcess 时才做进程搜索。
    #[test]
    fn test_route_enables_find_process_for_app_traffic() {
        let config = ConfigBuilder::new(vec![make_node("🇯🇵 日本-01")])
            .build()
            .expect("build config 应该成功");
        assert_eq!(
            config["route"]["find_process"], true,
            "必须开启 find_process，否则无 App-Matrix 规则时 processPath 恒空"
        );
    }
}
