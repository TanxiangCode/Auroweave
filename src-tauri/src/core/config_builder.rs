/// Sing-box config.json 动态配置生成器
/// 作者: TanXiang
///
/// 职责：
/// 1. 整理出站节点（去重、自动按地区分类成 urltest 分组）
/// 2. 构建 DNS 路由、RuleSet 和 Inbounds（Mixed 动态监听端口）
/// 3. 配置 ClashAPI 动态监听端口
use super::parser::ParsedOutbound;
use crate::error::AppError;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct ConfigBuilder {
    outbounds: Vec<ParsedOutbound>,
    mixed_port: u16,
    clash_api_port: u16,
}

impl ConfigBuilder {
    pub fn new(outbounds: Vec<ParsedOutbound>) -> Self {
        Self {
            outbounds,
            mixed_port: 7890,
            clash_api_port: 9090,
        }
    }

    pub fn with_ports(mut self, mixed_port: u16, clash_api_port: u16) -> Self {
        if mixed_port > 0 {
            self.mixed_port = mixed_port;
        }
        if clash_api_port > 0 {
            self.clash_api_port = clash_api_port;
        }
        self
    }

    /// 生成完整的 sing-box 1.11+ / 1.13+ / 1.14+ 兼容 config.json
    pub fn build(&self) -> Result<Value, AppError> {
        if self.outbounds.is_empty() {
            return Err(AppError::Config("没有可用节点，无法生成 config.json".to_string()));
        }

        let mut node_tags = Vec::new();
        let mut raw_outbounds = Vec::new();

        // 地区分组 mapping
        let mut region_map: HashMap<String, Vec<String>> = HashMap::new();

        for out in &self.outbounds {
            node_tags.push(out.tag.clone());
            raw_outbounds.push(out.raw_json.clone());

            let region = detect_region(&out.tag);
            region_map.entry(region).or_default().push(out.tag.clone());
        }

        let mut final_outbounds = Vec::new();

        // 1. Direct 与 Block 基础出站
        final_outbounds.push(json!({ "type": "direct", "tag": "direct" }));
        final_outbounds.push(json!({ "type": "block", "tag": "block" }));

        // 2. Selector "proxy" (主出站)
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

        // 3. 全局 "auto" urltest 出站
        final_outbounds.push(json!({
            "type": "urltest",
            "tag": "auto",
            "outbounds": node_tags,
            "url": "https://www.gstatic.com/generate_204",
            "interval": "15m",
            "idle_timeout": "30m"
        }));

        // 3.1 全局 "balance" 默认负载均衡/自动选择出站
        final_outbounds.push(json!({
            "type": "urltest",
            "tag": "balance",
            "outbounds": node_tags,
            "url": "https://www.gstatic.com/generate_204",
            "interval": "15m",
            "idle_timeout": "30m"
        }));

        // 4. 地区 urltest 出站
        for (region, tags) in region_map {
            final_outbounds.push(json!({
                "type": "urltest",
                "tag": format!("{}-auto", region),
                "outbounds": tags,
                "url": "https://www.gstatic.com/generate_204",
                "interval": "15m",
                "idle_timeout": "30m"
            }));
        }

        // 5. 节点具体出站
        final_outbounds.extend(raw_outbounds);

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

        let dns_rules = if server_domains.is_empty() {
            json!([])
        } else {
            json!([
                {
                    "domain": server_domains,
                    "server": "local"
                }
            ])
        };

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
                "final": "remote"
            },
            "inbounds": [
                {
                    "type": "mixed",
                    "tag": "mixed-in",
                    "listen": "127.0.0.1",
                    "listen_port": self.mixed_port
                }
            ],
            "outbounds": final_outbounds,
            "route": {
                "default_domain_resolver": "local",
                "rules": [
                    { "action": "sniff" },
                    { "protocol": "dns", "action": "hijack-dns" },
                    { "ip_is_private": true, "outbound": "direct" }
                ],
                "final": "proxy",
                "auto_detect_interface": true
            },
            "experimental": {
                "clash_api": {
                    "external_controller": format!("127.0.0.1:{}", self.clash_api_port),
                    "secret": ""
                }
            }
        });

        Ok(config)
    }
}

/// 识别节点 tag 属于哪个地区 (HK, JP, US, TW, SG, KR, OTHER)
fn detect_region(tag: &str) -> String {
    let lower = tag.to_lowercase();
    if lower.contains("hk") || lower.contains("香港") || tag.contains("🇭🇰") {
        "HK".to_string()
    } else if lower.contains("jp") || lower.contains("日本") || tag.contains("🇯🇵") {
        "JP".to_string()
    } else if lower.contains("us") || lower.contains("美国") || lower.contains("美國") || tag.contains("🇺🇸") {
        "US".to_string()
    } else if lower.contains("tw") || lower.contains("台湾") || lower.contains("臺灣") || tag.contains("🇹🇼") {
        "TW".to_string()
    } else if lower.contains("sg") || lower.contains("新加坡") || tag.contains("🇸🇬") {
        "SG".to_string()
    } else if lower.contains("kr") || lower.contains("韩国") || lower.contains("韓國") || tag.contains("🇰🇷") {
        "KR".to_string()
    } else {
        "OTHER".to_string()
    }
}
