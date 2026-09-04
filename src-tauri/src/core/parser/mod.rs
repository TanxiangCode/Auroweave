/// 订阅解析器入口
/// 作者: TanXiang
///
/// 支持格式自动识别与转换：
/// 1. Sing-box JSON 原生配置/节点数组
/// 2. Clash / Mihomo YAML
/// 3. V2ray / Base64 (vmess://, vless://, ss://, trojan://, hy2://)
pub mod clash;
pub mod v2ray;
pub mod singbox;

use crate::error::AppError;
use serde::{Deserialize, Serialize};

/// 格式化后的统一 sing-box outbound 抽象
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParsedOutbound {
    pub tag: String,
    pub r#type: String,
    pub server: Option<String>,
    pub server_port: Option<u16>,
    pub raw_json: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionFormat {
    SingboxJson,
    ClashYaml,
    Base64Uri,
    Unknown,
}

/// 自动检测格式并解析出 Outbound 列表
///
/// 各解析器依次尝试，失败时记录首个解析器的具体错误原因，
/// 在最终错误信息中携带，避免"解析失败"细节被逐层吞掉。
pub fn parse_subscription_content(content: &str) -> Result<(SubscriptionFormat, Vec<ParsedOutbound>), AppError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(AppError::Subscription("订阅内容为空".to_string()));
    }

    // 首个解析器的具体失败原因（最终错误中携带）
    let mut first_err: Option<String> = None;

    // 1. 尝试 Sing-box JSON
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        match singbox::parse_singbox_json(trimmed) {
            Ok(outbounds) => return Ok((SubscriptionFormat::SingboxJson, outbounds)),
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(format!("singbox JSON: {}", e));
                }
            }
        }
    }

    // 2. 尝试 Clash YAML
    if trimmed.contains("proxies:") || trimmed.contains("Proxy:") {
        match clash::parse_clash_yaml(trimmed) {
            Ok(outbounds) => return Ok((SubscriptionFormat::ClashYaml, outbounds)),
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(format!("clash YAML: {}", e));
                }
            }
        }
    }

    // 3. 尝试 Base64 / URI 列表（失败不产生有意义的错误信息，跳过记录）
    if let Ok(outbounds) = v2ray::parse_v2ray_base64(trimmed) {
        if !outbounds.is_empty() {
            return Ok((SubscriptionFormat::Base64Uri, outbounds));
        }
    }

    // 兜底尝试 YAML
    match clash::parse_clash_yaml(trimmed) {
        Ok(outbounds) => {
            if !outbounds.is_empty() {
                return Ok((SubscriptionFormat::ClashYaml, outbounds));
            }
        }
        Err(e) => {
            if first_err.is_none() {
                first_err = Some(format!("clash YAML: {}", e));
            }
        }
    }

    let detail = first_err.unwrap_or_else(|| "所有解析器均未产出有效节点".to_string());
    Err(AppError::Subscription(format!("未知的订阅格式或解析失败（{}）", detail)))
}

/// 检查节点是否属于机场公告、流量信息、到期提示等非真实代理节点
///
/// 判定策略（宁可漏判公告节点，不可误杀真实节点）：
/// 1. 占位服务器地址 + 占位端口
/// 2. 强特征关键词：任意命中即判伪（"到期"、"剩余"、"流量"、"官网"、
///    "网址"、"发布"、"更新"、"订阅"、"expire"、"traffic"）
/// 3. 域名 TLD 结尾：tag 以 ".com"/".cc"/".org"/".net"/".xyz"/".top" 结尾
///    （要求出现在末尾，避免误杀名称中含 ".com" 的真实节点）
/// 4. 弱特征关键词分两组，单组内命中 ≥2 个才判伪（单独出现不足以定性）
///    - 组1 社群/引导类：推荐、群、group、频道、channel、官方、关注、
///      认准、教程、请勿、通知、发布页
///    - 组2 状态/地址类：reset、remain、已用、套餐、有效、测速、阻断、
///      http://、https://
/// 5. 公告 Emoji 提示符开头
pub fn is_announcement_or_fake_node(tag: &str, server: Option<&str>, port: Option<u16>) -> bool {
    let lower_tag = tag.to_lowercase();

    // 1. 检查服务器地址是否为常见占位符
    if let Some(srv) = server {
        let trimmed_srv = srv.trim();
        if trimmed_srv == "1.1.1.1" || trimmed_srv == "127.0.0.1" || trimmed_srv == "0.0.0.0" || trimmed_srv == "240.0.0.1" {
            // 如果端口为 0 或 8888 / 1234 等非标准占位端口，判定为伪节点
            if let Some(p) = port {
                if p == 0 || p == 8888 || p == 1234 || p == 500 {
                    return true;
                }
            }
        }
    }

    // 2. 强特征关键词：单命中即判伪
    let strong_keywords = [
        "到期", "剩余", "流量", "官网", "网址",
        "发布", "更新", "订阅", "expire", "traffic",
    ];
    if strong_keywords.iter().any(|kw| lower_tag.contains(kw)) {
        return true;
    }

    // 3. 域名 TLD 类关键词：必须出现在 tag 末尾（trim 尾部空白后 ends_with 匹配），
    //    避免误杀名称中间含 ".com" 等字样的真实节点
    let trimmed_lower_tag = lower_tag.trim_end();
    let tld_keywords = [".com", ".cc", ".org", ".net", ".xyz", ".top"];
    if tld_keywords.iter().any(|kw| trimmed_lower_tag.ends_with(kw)) {
        return true;
    }

    // 4. 弱特征关键词分两组：单组内命中 >= 2 个才判伪
    //    组1: 社群/引流类词汇，组2: 状态/地址类词汇
    let group1 = [
        "推荐", "群", "group", "频道", "channel", "官方",
        "关注", "认准", "教程", "请勿", "通知", "发布页",
    ];
    let group2 = [
        "reset", "remain", "已用", "套餐", "有效",
        "测速", "阻断", "http://", "https://",
    ];
    let g1_hits = group1.iter().filter(|kw| lower_tag.contains(**kw)).count();
    if g1_hits >= 2 {
        return true;
    }
    let g2_hits = group2.iter().filter(|kw| lower_tag.contains(**kw)).count();
    if g2_hits >= 2 {
        return true;
    }

    // 5. 检查是否以特定 Emoji 提示符开头
    if tag.starts_with("⚠️") || tag.starts_with("📢") || tag.starts_with("📌") || tag.starts_with("ℹ️") {
        return true;
    }

    false
}

/// 判断 ParsedOutbound 是否为真实有效的代理节点
pub fn is_valid_proxy_node(out: &ParsedOutbound) -> bool {
    // 基础协议检查（snell 自 sing-box 1.14.0 起原生支持）
    let valid_types = ["ss", "shadowsocks", "vmess", "vless", "trojan", "hysteria2", "hy2", "tuic", "wireguard", "socks", "http", "anytls", "snell", "hysteria"];
    if !valid_types.contains(&out.r#type.to_lowercase().as_str()) {
        return false;
    }

    !is_announcement_or_fake_node(&out.tag, out.server.as_deref(), out.server_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_announcement_or_fake_node() {
        assert!(is_announcement_or_fake_node("认准官网地址", Some("zf-tw1.9999231.xyz"), Some(500)));
        assert!(is_announcement_or_fake_node("SKYLUMO.CC", Some("zf-tw1.9999231.xyz"), Some(500)));
        assert!(is_announcement_or_fake_node("⚠️请勿全批量测速", Some("zf-tw1.9999231.xyz"), Some(500)));
        assert!(is_announcement_or_fake_node("套餐到期：永久有效", Some("zf-tw1.9999231.xyz"), Some(500)));
        assert!(is_announcement_or_fake_node("剩余流量：9999214.57 GB", Some("zf-tw1.9999231.xyz"), Some(500)));
        assert!(is_announcement_or_fake_node("⚠️ 如果现在只能看到少数线路", Some("1.1.1.1"), Some(8888)));

        // 真实节点不应判定为假节点
        assert!(!is_announcement_or_fake_node("🇨🇳 台湾-住宅家宽-001", Some("zf-tw1.9999231.xyz"), Some(1001)));
        assert!(!is_announcement_or_fake_node("🇯🇵 日本-极速-001", Some("zf-jp.9999231.xyz"), Some(1004)));
        assert!(!is_announcement_or_fake_node("🇺🇸 美国-下载专用-001", Some("zf-tw1.9999231.xyz"), Some(1002)));
    }

    #[test]
    fn test_fake_node_keyword_conservative() {
        // TLD 结尾匹配：域名形式结尾判伪
        assert!(is_announcement_or_fake_node("SKYLUMO.CC", Some("srv.example.com"), Some(443)));
        assert!(is_announcement_or_fake_node("官网 example.com", Some("srv.example.com"), Some(443)));

        // 名称中间含 ".com" 但不以之结尾的真实节点不应误杀
        assert!(!is_announcement_or_fake_node("IEPL-aws.com专线-东京-01", Some("srv.example.com"), Some(443)));

        // 单独弱关键词不足以判伪（避免误杀真实节点）
        assert!(!is_announcement_or_fake_node("香港-推荐线路-01", Some("srv.example.com"), Some(443)));
        assert!(!is_announcement_or_fake_node("新加坡-群组高速-01", Some("srv.example.com"), Some(443)));

        // 同组内两个弱关键词组合才判伪
        assert!(is_announcement_or_fake_node("推荐加入TG群", Some("srv.example.com"), Some(443)));
        assert!(is_announcement_or_fake_node("已用流量套餐查询", Some("srv.example.com"), Some(443)));

        // 强关键词单命中即判伪
        assert!(is_announcement_or_fake_node("距离到期还有7天", Some("srv.example.com"), Some(443)));
        assert!(is_announcement_or_fake_node("expire in 3 days", Some("srv.example.com"), Some(443)));
    }

    #[test]
    fn test_parse_subscription_content_carries_first_error() {
        // 格式非法但携带首个解析器的具体错误
        let result = parse_subscription_content("proxies: 123");
        assert!(result.is_err());
        let msg = format!("{}", result.err().unwrap());
        assert!(msg.contains("clash YAML"), "最终错误应携带首个解析器错误，实际: {}", msg);
    }
}

