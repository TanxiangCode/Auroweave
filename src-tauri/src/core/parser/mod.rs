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
pub fn parse_subscription_content(content: &str) -> Result<(SubscriptionFormat, Vec<ParsedOutbound>), AppError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(AppError::Subscription("订阅内容为空".to_string()));
    }

    // 1. 尝试 Sing-box JSON
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(outbounds) = singbox::parse_singbox_json(trimmed) {
            return Ok((SubscriptionFormat::SingboxJson, outbounds));
        }
    }

    // 2. 尝试 Clash YAML
    if trimmed.contains("proxies:") || trimmed.contains("Proxy:") {
        if let Ok(outbounds) = clash::parse_clash_yaml(trimmed) {
            return Ok((SubscriptionFormat::ClashYaml, outbounds));
        }
    }

    // 3. 尝试 Base64 / URI 列表
    if let Ok(outbounds) = v2ray::parse_v2ray_base64(trimmed) {
        if !outbounds.is_empty() {
            return Ok((SubscriptionFormat::Base64Uri, outbounds));
        }
    }

    // 兜底尝试 YAML
    if let Ok(outbounds) = clash::parse_clash_yaml(trimmed) {
        if !outbounds.is_empty() {
            return Ok((SubscriptionFormat::ClashYaml, outbounds));
        }
    }

    Err(AppError::Subscription("未知的订阅格式或解析失败".to_string()))
}

/// 检查节点是否属于机场公告、流量信息、到期提示等非真实代理节点
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

    // 2. 检查名称中是否包含公告、提示、流量等特征关键字
    let fake_keywords = [
        "官网", "到期", "流量", "剩余", "已用", "套餐", "通知",
        "官方", "发布页", "请勿", "更新", "有效", "关注", "网址",
        "认准", "测速", "阻断", "群", "频道", "教程", "推荐",
        "expire", "traffic", "reset", "remain", "channel", "group",
        "http://", "https://", ".com", ".cc", ".org", ".net", ".xyz", ".top"
    ];

    for kw in &fake_keywords {
        if lower_tag.contains(kw) {
            return true;
        }
    }

    // 3. 检查是否以特定 Emoji 提示符开头
    if tag.starts_with("⚠️") || tag.starts_with("📢") || tag.starts_with("📌") || tag.starts_with("ℹ️") {
        return true;
    }

    false
}

/// 判断 ParsedOutbound 是否为真实有效的代理节点
pub fn is_valid_proxy_node(out: &ParsedOutbound) -> bool {
    // 基础协议检查
    let valid_types = ["ss", "shadowsocks", "vmess", "vless", "trojan", "hysteria2", "hy2", "tuic", "wireguard", "socks", "http", "anytls"];
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
}

