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
use super::parser::ParsedOutbound;
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
                    "server": "8.8.8.8",
                    "tag": "remote",
                    "type": "udp"
                },
                {
                    "server": "223.5.5.5",
                    "tag": "local",
                    "type": "udp"
                }
            ],
            "rules": [],
            "final": "remote",
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
                { "ip_is_private": true, "outbound": "direct" }
            ],
            "final": "direct",
            "auto_detect_interface": true
        },
        "experimental": {
            "clash_api": {
                "external_controller": format!("127.0.0.1:{}", clash_api_port),
                "secret": ""
            },
            "cache_file": {
                "enabled": true,
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
        }
    }

    /// 设置局域网共享模式
    pub fn with_allow_lan(mut self, allow_lan: bool) -> Self {
        self.allow_lan = allow_lan;
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

    /// 生成完整的 sing-box 1.11+ / 1.13+ / 1.14+ 兼容 config.json
    pub fn build(&self) -> Result<Value, AppError> {
        if self.outbounds.is_empty() {
            return Err(AppError::Config("没有可用节点，无法生成 config.json".to_string()));
        }

        // ---- 阶段1: 遍历节点，提取 tag 和原始 JSON，按地区分组 ----
        let mut node_tags = Vec::new();
        let mut raw_outbounds = Vec::new();
        let mut region_map: HashMap<String, Vec<String>> = HashMap::new();

        for out in &self.outbounds {
            node_tags.push(out.tag.clone());
            raw_outbounds.push(out.raw_json.clone());
            let region = detect_region(&out.tag);
            region_map.entry(region).or_default().push(out.tag.clone());
        }

        let mut final_outbounds = Vec::new();

        // ---- 阶段2: 构建出站列表 ----

        // 2a. Direct 与 Block 基础出站
        final_outbounds.push(json!({ "type": "direct", "tag": "direct" }));
        final_outbounds.push(json!({ "type": "block", "tag": "block" }));

        // 2b. Selector "proxy" 主出站
        let mut proxy_group_list = vec!["auto".to_string(), "balance".to_string()];
        for (region, _) in &region_map {
            proxy_group_list.push(format!("{}-auto", region));
        }
        proxy_group_list.extend(node_tags.clone());

        final_outbounds.push(json!({
            "type": "selector",
            "tag": "proxy",
            "outbounds": proxy_group_list
        }));

        // 2c. 全局 "auto" urltest 出站（自动测速选最优节点，3 分钟心跳，50ms 容差）
        final_outbounds.push(json!({
            "type": "urltest",
            "tag": "auto",
            "outbounds": node_tags.clone(),
            "url": "http://www.gstatic.com/generate_204",
            "interval": "3m",
            "idle_timeout": "15m",
            "tolerance": 50,
            "interrupt_exist_connections": false
        }));

        // 2c-2. "balance" 负载均衡出站（urltest + 短间隔 + tolerance，近似负载均衡效果）
        final_outbounds.push(json!({
            "type": "urltest",
            "tag": "balance",
            "outbounds": node_tags,
            "url": "http://www.gstatic.com/generate_204",
            "interval": "3m",
            "idle_timeout": "10m",
            "tolerance": 50,
            "interrupt_exist_connections": false
        }));

        // 2d. 地区 urltest 出站（3 分钟自动探测最优节点）
        for (region, tags) in region_map {
            final_outbounds.push(json!({
                "type": "urltest",
                "tag": format!("{}-auto", region),
                "outbounds": tags,
                "url": "http://www.gstatic.com/generate_204",
                "interval": "3m",
                "idle_timeout": "15m",
                "tolerance": 50,
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

    let mut dns_rules = if server_domains.is_empty() {
        vec![]
    } else {
        vec![json!({ "domain": server_domains, "server": "local" })]
    };

    // 测速与健康检测域名强制使用 local DNS 解析，避免未选定节点时依赖 remote/proxy 产生 DNS 死锁
    dns_rules.push(json!({
        "domain": [
            "www.gstatic.com",
            "connectivitycheck.gstatic.com",
            "cp.cloudflare.com",
            "msftconnecttest.com"
        ],
        "server": "local"
    }));

    // 仅当 geosite-cn rule-set 可用时才添加 DNS 规则
    if self.geosite_cn_path.is_some() {
        dns_rules.push(json!({ "rule_set": "geosite-cn", "server": "local" }));
    }


        // ---- 阶段4: 组装路由 ----
        // 检查本地 rule-set 文件是否存在
        let has_geosite = self.geosite_cn_path.as_ref().map(|p| std::path::Path::new(p).exists()).unwrap_or(false);
        let has_geoip = self.geoip_cn_path.as_ref().map(|p| std::path::Path::new(p).exists()).unwrap_or(false);

        let mut rule_set_config = Vec::new();
        let mut route_rules = json!([
            { "action": "sniff" },
            { "protocol": "dns", "action": "hijack-dns" },
            { "ip_is_private": true, "outbound": "direct" }
        ]);

        // 注入 App-Matrix 应用分流规则 (优先级高于通用域名分流)
        let app_rules = crate::commands::routing::load_app_rules_internal();
        if let Some(rules) = route_rules.as_array_mut() {
            for (proc_name, outbound) in app_rules {
                if !proc_name.is_empty() && !outbound.is_empty() {
                    rules.push(json!({
                        "process_name": [proc_name],
                        "outbound": outbound
                    }));
                }
            }
        }

        // 注入自定义域名 / IP 分流规则 (仅注入 enabled == true)
        let custom_rules = crate::commands::routing::load_custom_rules_internal();
        if let Some(rules) = route_rules.as_array_mut() {
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
                        rules.push(json!({ "ip_cidr": [payload], "outbound": outbound }));
                    }
                    _ => {
                        rules.push(json!({ "domain_suffix": [payload], "outbound": outbound }));
                    }
                }
            }
        }


        if has_geosite {
            rule_set_config.push(json!({
                "tag": "geosite-cn",
                "type": "local",
                "format": "binary",
                "path": self.geosite_cn_path.as_ref().unwrap()
            }));
            // 添加 geosite-cn 直连规则
            if let Some(rules) = route_rules.as_array_mut() {
                rules.push(json!({ "rule_set": ["geosite-cn"], "outbound": "direct" }));
            }
            if has_geoip {
                rule_set_config.push(json!({
                    "tag": "geoip-cn",
                    "type": "local",
                    "format": "binary",
                    "path": self.geoip_cn_path.as_ref().unwrap()
                }));
                // 合并 geosite-cn + geoip-cn 直连规则
                if let Some(rules) = route_rules.as_array_mut() {
                    let _ = rules.pop().unwrap();
                    // 替换为合并规则
                    rules.push(json!({ "rule_set": ["geosite-cn", "geoip-cn"], "outbound": "direct" }));
                }
            }
        } else {
            log::warn!("[config] geosite-cn.srs 本地文件不存在，跳过国内域名直连规则，所有流量走代理");
        }

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
                        "server": "8.8.8.8",
                        "tag": "remote",
                        "type": "udp"
                    },
                    {
                        "server": "223.5.5.5",
                        "tag": "local",
                        "type": "udp"
                    }
                ],
                "rules": dns_rules,
                "final": "remote",
                "strategy": "prefer_ipv4"
            },
            "inbounds": [
                {
                    "type": "mixed",
                    "tag": "mixed-in",
                    "listen": if self.allow_lan { "0.0.0.0" } else { "127.0.0.1" },
                    "listen_port": self.mixed_port
                }
            ],

            "outbounds": final_outbounds,
            "route": route,
            "experimental": {
                "clash_api": {
                    "external_controller": format!("127.0.0.1:{}", self.clash_api_port),
                    "secret": ""
                },
                "cache_file": {
                    "enabled": true,
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
