/// Sing-box config.json 动态配置生成器
/// 作者: TanXiang
///
/// 职责：
/// 1. 整理出站节点（去重、自动按地区分类成 urltest 分组）
/// 2. 构建 DNS 路由、RuleSet 和 Inbounds（Mixed 7890 端口）
/// 3. 配置 ClashAPI (127.0.0.1:9090)
use super::parser::ParsedOutbound;
use crate::error::AppError;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct ConfigBuilder {
    outbounds: Vec<ParsedOutbound>,
}

impl ConfigBuilder {
    pub fn new(outbounds: Vec<ParsedOutbound>) -> Self {
        Self { outbounds }
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

            // 识别节点地区并分类
            let region = detect_region(&out.tag);
            region_map.entry(region).or_default().push(out.tag.clone());
        }

        let mut final_outbounds = Vec::new();

        // 1. Selector "proxy" (主出站)
        let mut proxy_group_list = vec!["auto".to_string()];
        for (region, _) in &region_map {
            proxy_group_list.push(format!("{}-auto", region));
        }
        proxy_group_list.extend(node_tags.clone());

        final_outbounds.push(json!({
            "type": "selector",
            "tag": "proxy",
            "outbounds": proxy_group_list
        }));

        // 2. 全局 "auto" urltest 出站
        final_outbounds.push(json!({
            "type": "urltest",
            "tag": "auto",
            "outbounds": node_tags,
            "url": "https://www.gstatic.com/generate_204",
            "interval": "5m"
        }));

        // 3. 地区 urltest 出站
        for (region, tags) in region_map {
            final_outbounds.push(json!({
                "type": "urltest",
                "tag": format!("{}-auto", region),
                "outbounds": tags,
                "url": "https://www.gstatic.com/generate_204",
                "interval": "5m"
            }));
        }

        // 4. 节点具体出站
        final_outbounds.extend(raw_outbounds);

        // 5. 补全直连和拦截
        final_outbounds.push(json!({ "type": "direct", "tag": "direct" }));
        final_outbounds.push(json!({ "type": "block", "tag": "block" }));

        let config = json!({
            "log": {
                "level": "info",
                "output": "box.log",
                "timestamp": true
            },
            "dns": {
                "servers": [
                    {
                        "tag": "remote",
                        "type": "https",
                        "server": "1.1.1.1",
                        "path": "/dns-query",
                        "detour": "proxy"
                    },
                    {
                        "tag": "local",
                        "type": "udp",
                        "server": "223.5.5.5",
                        "detour": "direct"
                    }
                ],
                "rules": [],
                "final": "remote"
            },
            "inbounds": [
                {
                    "type": "mixed",
                    "tag": "mixed-in",
                    "listen": "127.0.0.1",
                    "listen_port": 7890
                }
            ],
            "outbounds": final_outbounds,
            "route": {
                "default_domain_resolver": "remote",
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
                    "external_controller": "127.0.0.1:9090",
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
