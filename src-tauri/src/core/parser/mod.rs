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
