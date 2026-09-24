/// Sing-box 原生 JSON 订阅解析器
/// 作者: TanXiang
use super::ParsedOutbound;
use crate::error::AppError;
use serde_json::Value;

/// sing-box 1.14 原生 outbound 类型白名单。URI 别名（ss/hy2）和已迁移的
/// wireguard/loadbalance 不属于原生配置 type，不能直接透传。
const CANONICAL_OUTBOUND_TYPES: &[&str] = &[
    "anytls",
    "bridge",
    "hysteria",
    "hysteria2",
    "naive",
    "shadowsocks",
    "shadowtls",
    "snell",
    "socks",
    "ssh",
    "tor",
    "trojan",
    "tuic",
    "vless",
    "vmess",
];

fn subscription_error(tag: &str, message: impl std::fmt::Display) -> AppError {
    AppError::Subscription(format!("Singbox JSON 节点 [{}] {}", tag, message))
}

fn required_string(
    item: &Value,
    field: &str,
    tag: &str,
    type_name: &str,
) -> Result<String, AppError> {
    item.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| subscription_error(tag, format!("缺少必填字段 {} ({})", field, type_name)))
}

fn validate_server_port(
    item: &Value,
    tag: &str,
    type_name: &str,
    required: bool,
) -> Result<Option<u16>, AppError> {
    let Some(value) = item.get("server_port") else {
        if required {
            return Err(subscription_error(
                tag,
                format!("缺少必填字段 server_port ({})", type_name),
            ));
        }
        return Ok(None);
    };
    let port = value.as_u64().ok_or_else(|| {
        subscription_error(
            tag,
            format!("server_port 必须为 1..=65535 的整数 ({})", type_name),
        )
    })?;
    if !(1..=65535).contains(&port) {
        return Err(subscription_error(
            tag,
            format!("server_port 超出 1..=65535 ({})", type_name),
        ));
    }
    Ok(Some(port as u16))
}

pub fn parse_singbox_json(content: &str) -> Result<Vec<ParsedOutbound>, AppError> {
    let parsed: Value = serde_json::from_str(content)
        .map_err(|e| AppError::Subscription(format!("Singbox JSON 语法错误: {}", e)))?;

    let mut outbounds = Vec::new();
    if let Some(arr) = parsed.as_array() {
        for item in arr {
            if let Some(outbound) = extract_outbound(item)? {
                outbounds.push(outbound);
            }
        }
    } else if let Some(outbound_list) = parsed.get("outbounds").and_then(Value::as_array) {
        for item in outbound_list {
            if let Some(outbound) = extract_outbound(item)? {
                outbounds.push(outbound);
            }
        }
    }

    if outbounds.is_empty() {
        return Err(AppError::Subscription(
            "Singbox 配置中没有找到有效节点".to_string(),
        ));
    }
    Ok(outbounds)
}

fn extract_outbound(item: &Value) -> Result<Option<ParsedOutbound>, AppError> {
    let tag = item
        .get("tag")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Subscription("Singbox JSON 节点缺少 tag".to_string()))?
        .to_string();
    let type_name = item
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| subscription_error(&tag, "缺少 type 字段"))?
        .to_string();

    // 辅助出站不是订阅节点，跳过而不是对其套用节点必填字段校验。
    if matches!(
        type_name.as_str(),
        "direct" | "block" | "dns" | "selector" | "urltest"
    ) {
        return Ok(None);
    }
    if !CANONICAL_OUTBOUND_TYPES.contains(&type_name.as_str()) {
        return Err(subscription_error(
            &tag,
            format!("不支持的 outbound type [{}]", type_name),
        ));
    }

    let server = item
        .get("server")
        .and_then(Value::as_str)
        .map(str::to_string);
    let server_port = if matches!(type_name.as_str(), "bridge" | "tor") {
        None
    } else if type_name == "hysteria2" && item.get("server_ports").is_some() {
        // Hysteria2 端口跳跃与 server_port 互斥。
        None
    } else {
        validate_server_port(item, &tag, &type_name, true)?
    };

    match type_name.as_str() {
        "vless" | "vmess" => {
            required_string(item, "uuid", &tag, &type_name)?;
        }
        "trojan" | "hysteria2" | "anytls" => {
            required_string(item, "password", &tag, &type_name)?;
        }
        "shadowsocks" => {
            required_string(item, "method", &tag, &type_name)?;
            required_string(item, "password", &tag, &type_name)?;
        }
        "naive" => {
            required_string(item, "username", &tag, &type_name)?;
            required_string(item, "password", &tag, &type_name)?;
        }
        "shadowtls" => {
            if !item.get("version").and_then(Value::as_u64).is_some() {
                return Err(subscription_error(&tag, "缺少必填字段 version"));
            }
            required_string(item, "password", &tag, &type_name)?;
        }
        "snell" => {
            if !item.get("version").and_then(Value::as_u64).is_some() {
                return Err(subscription_error(&tag, "缺少必填字段 version"));
            }
            required_string(item, "psk", &tag, &type_name)?;
        }
        "tuic" => {
            required_string(item, "uuid", &tag, &type_name)?;
        }
        "ssh" => {
            required_string(item, "user", &tag, &type_name)?;
            if item
                .get("password")
                .and_then(Value::as_str)
                .unwrap_or("")
                .is_empty()
                && item
                    .get("private_key")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .is_empty()
                && item
                    .get("private_key_path")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .is_empty()
            {
                return Err(subscription_error(
                    &tag,
                    "ssh 必须提供 password/private_key/private_key_path 之一",
                ));
            }
        }
        "tor" => {
            required_string(item, "executable_path", &tag, &type_name)?;
        }
        _ => {}
    }

    let server = match type_name.as_str() {
        "bridge" | "tor" => None,
        _ => Some(server.ok_or_else(|| subscription_error(&tag, "缺少必填字段 server"))?),
    };

    Ok(Some(ParsedOutbound {
        tag,
        r#type: type_name,
        server,
        server_port,
        raw_json: item.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_one(value: Value) -> Result<Vec<ParsedOutbound>, AppError> {
        parse_singbox_json(&value.to_string())
    }

    #[test]
    fn accepts_canonical_vless_and_skips_auxiliary() {
        let parsed = parse_one(json!({
            "outbounds": [
                {"type": "direct", "tag": "direct"},
                {"type": "selector", "tag": "proxy", "outbounds": []},
                {"type": "vless", "tag": "v", "server": "example.com", "server_port": 443,
                 "uuid": "bf000d23-0752-40b4-affe-68f7707a9661"}
            ]
        }))
        .unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].r#type, "vless");
    }

    #[test]
    fn rejects_alias_unknown_type_and_invalid_port() {
        for value in [
            json!({"type": "ss", "tag": "ss", "server": "example.com", "server_port": 443,
                   "method": "aes-128-gcm", "password": "x"}),
            json!({"type": "unknown", "tag": "x", "server": "example.com", "server_port": 443}),
            json!({"type": "vless", "tag": "v", "server": "example.com", "server_port": 70000,
                   "uuid": "bf000d23-0752-40b4-affe-68f7707a9661"}),
        ] {
            assert!(parse_one(value).is_err());
        }
    }

    #[test]
    fn rejects_missing_required_fields() {
        assert!(parse_one(
            json!({"type": "trojan", "tag": "t", "server": "example.com", "server_port": 443})
        )
        .is_err());
        assert!(parse_one(
            json!({"type": "vless", "tag": "v", "server": "example.com", "server_port": 443})
        )
        .is_err());
    }
}
