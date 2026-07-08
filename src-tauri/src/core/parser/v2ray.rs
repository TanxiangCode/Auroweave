/// V2Ray / Base64 / URI 列表解析器
/// 支持: vmess://, vless://, ss://, trojan://, hysteria2://, hy2://, anytls://
/// 作者: TanXiang
use super::ParsedOutbound;
use crate::error::AppError;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::json;
use url::Url;

pub fn parse_v2ray_base64(content: &str) -> Result<Vec<ParsedOutbound>, AppError> {
    let clean = content.trim().replace("\r\n", "\n").replace('\r', "");
    let decoded = flexible_base64_decode(&clean).unwrap_or_else(|| clean.clone());

    let lines = decoded.lines();
    let mut outbounds = Vec::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(parsed) = parse_single_uri(line) {
            outbounds.push(parsed);
        }
    }

    Ok(outbounds)
}

/// 兼容多种 Base64 编码变体与补齐
fn flexible_base64_decode(input: &str) -> Option<String> {
    let sanitized = input.replace([' ', '\n', '\t'], "");
    if sanitized.is_empty() {
        return None;
    }

    // 补全 Base64 填充符 '='
    let mut padded = sanitized.clone();
    let rem = padded.len() % 4;
    if rem > 0 {
        padded.push_str(&"=".repeat(4 - rem));
    }

    if let Ok(bytes) = STANDARD.decode(&padded) {
        if let Ok(s) = String::from_utf8(bytes) { return Some(s); }
    }
    if let Ok(bytes) = URL_SAFE.decode(&padded) {
        if let Ok(s) = String::from_utf8(bytes) { return Some(s); }
    }
    if let Ok(bytes) = STANDARD_NO_PAD.decode(&sanitized) {
        if let Ok(s) = String::from_utf8(bytes) { return Some(s); }
    }
    if let Ok(bytes) = URL_SAFE_NO_PAD.decode(&sanitized) {
        if let Ok(s) = String::from_utf8(bytes) { return Some(s); }
    }

    None
}

fn parse_single_uri(uri: &str) -> Option<ParsedOutbound> {
    if uri.starts_with("vmess://") {
        parse_vmess_uri(uri)
    } else if uri.starts_with("ss://") {
        parse_ss_uri(uri)
    } else if uri.starts_with("trojan://") {
        parse_trojan_uri(uri)
    } else if uri.starts_with("vless://") {
        parse_vless_uri(uri)
    } else if uri.starts_with("hysteria2://") || uri.starts_with("hy2://") {
        parse_hysteria2_uri(uri)
    } else if uri.starts_with("anytls://") {
        parse_anytls_uri(uri)
    } else {
        None
    }
}

/// 解码 URL Fragment 中的 percent-encoding 标签名 (如 %F0%9F%87%BA%F0%9F%87%B8 -> 🇺🇸)
fn extract_tag(parsed_url: &Url, default_name: &str) -> String {
    if let Some(fragment) = parsed_url.fragment() {
        if !fragment.is_empty() {
            return urlencoding::decode(fragment)
                .unwrap_or_else(|_| fragment.into())
                .to_string();
        }
    }
    default_name.to_string()
}

fn parse_hysteria2_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "Hysteria2");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let password = parsed_url.username().to_string();

    let mut sni = None;
    let mut insecure = false;
    let mut obfs_type = None;
    let mut obfs_pass = None;

    for (k, v) in parsed_url.query_pairs() {
        match k.as_ref() {
            "sni" => sni = Some(v.to_string()),
            "insecure" => insecure = v == "1" || v == "true",
            "obfs" => obfs_type = Some(v.to_string()),
            "obfs-password" => {
                obfs_pass = Some(
                    urlencoding::decode(&v)
                        .map(|cow| cow.into_owned())
                        .unwrap_or_else(|_| v.to_string()),
                );
            }
            _ => {}
        }
    }

    let mut raw_json = json!({
        "type": "hysteria2",
        "tag": tag,
        "server": server,
        "server_port": port,
        "password": password
    });

    if sni.is_some() || insecure {
        let mut tls = json!({ "enabled": true });
        if let Some(s) = sni { tls["server_name"] = json!(s); }
        if insecure { tls["insecure"] = json!(true); }
        raw_json["tls"] = tls;
    }

    if let Some(o_type) = obfs_type {
        let mut obfs = json!({ "type": o_type });
        if let Some(p) = obfs_pass { obfs["password"] = json!(p); }
        raw_json["obfs"] = obfs;
    }

    Some(ParsedOutbound {
        tag: extract_tag(&parsed_url, "Hysteria2"),
        r#type: "hysteria2".to_string(),
        server: Some(server),
        server_port: Some(port),
        raw_json,
    })
}

fn parse_anytls_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "AnyTLS");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let uuid = parsed_url.username().to_string();

    let mut sni = None;
    let mut insecure = false;

    for (k, v) in parsed_url.query_pairs() {
        match k.as_ref() {
            "sni" => sni = Some(v.to_string()),
            "insecure" => insecure = v == "1" || v == "true",
            _ => {}
        }
    }

    let mut tls = json!({ "enabled": true });
    if let Some(s) = sni { tls["server_name"] = json!(s); }
    if insecure { tls["insecure"] = json!(true); }

    let raw_json = json!({
        "type": "vless",
        "tag": tag,
        "server": server,
        "server_port": port,
        "uuid": uuid,
        "tls": tls
    });

    Some(ParsedOutbound {
        tag: extract_tag(&parsed_url, "AnyTLS"),
        r#type: "vless".to_string(),
        server: Some(server),
        server_port: Some(port),
        raw_json,
    })
}

fn parse_vmess_uri(uri: &str) -> Option<ParsedOutbound> {
    let b64_str = uri.strip_prefix("vmess://")?;
    let decoded_bytes = flexible_base64_decode(b64_str).or_else(|| {
        STANDARD.decode(b64_str).ok().and_then(|b| String::from_utf8(b).ok())
    })?;

    let v: serde_json::Value = serde_json::from_str(&decoded_bytes).ok()?;

    let name = v.get("ps").and_then(|s| s.as_str()).unwrap_or("VMess").to_string();
    let server = v.get("add")?.as_str()?.to_string();
    let port = v.get("port")?.as_str()?.parse::<u16>().ok()?;
    let uuid = v.get("id")?.as_str()?.to_string();

    Some(ParsedOutbound {
        tag: name.clone(),
        r#type: "vmess".to_string(),
        server: Some(server.clone()),
        server_port: Some(port),
        raw_json: json!({
            "type": "vmess",
            "tag": name,
            "server": server,
            "server_port": port,
            "uuid": uuid,
            "security": "auto"
        }),
    })
}

fn parse_ss_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "Shadowsocks");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let user_info = parsed_url.username();

    Some(ParsedOutbound {
        tag: tag.clone(),
        r#type: "shadowsocks".to_string(),
        server: Some(server.clone()),
        server_port: Some(port),
        raw_json: json!({
            "type": "shadowsocks",
            "tag": tag,
            "server": server,
            "server_port": port,
            "method": "aes-256-gcm",
            "password": user_info
        }),
    })
}

fn parse_trojan_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "Trojan");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let password = parsed_url.username();

    Some(ParsedOutbound {
        tag: tag.clone(),
        r#type: "trojan".to_string(),
        server: Some(server.clone()),
        server_port: Some(port),
        raw_json: json!({
            "type": "trojan",
            "tag": tag,
            "server": server,
            "server_port": port,
            "password": password
        }),
    })
}

fn parse_vless_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "VLESS");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let uuid = parsed_url.username();

    Some(ParsedOutbound {
        tag: tag.clone(),
        r#type: "vless".to_string(),
        server: Some(server.clone()),
        server_port: Some(port),
        raw_json: json!({
            "type": "vless",
            "tag": tag,
            "server": server,
            "server_port": port,
            "uuid": uuid
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_user_subscription() {
        let sample = "YW55dGxzOi8vYmQ5NDEwZmItZDgyOS00ODI3LWIzYWQtNzAwMzlmMjI4YjVmQHVzYS45OTY2NjkwLnh5ejo1MDAxLz90eXBlPXRjcCZpbnNlY3VyZT0xJmZwPWNocm9tZSZzbmk9aW9zYXBwcy5pdHVuZXMuYXBwbGUuY29tIyVFNSU4OSVBOSVFNCVCRCU5OSVFNiVCNSU4MSVFOSU4NyU4RiVFRiVCQyU5OTk5OTkyNzIuOTUlMjBHQg0KaHlzdGVyaWEyOi8vYmQ5NDEwZmItZDgyOS00ODI3LWIzYWQtNzAwMzlmMjI4YjVmQHVzYS45OTY2Njkw.eHl6OjEwMDAwLz9pbnNlY3VyZT0xJnNuaT1pb3NhcHBzLml0dW5lcy5hcHBsZS5jb20mb2Jmcz1zYWxhbWFuZGVyJm9iZnMtcGFzc3dvcmQ9WXpneU9Ua3dORGs0WlRVMk5UZGlOQSUzRCUzRCZtcG9ydD0xMDAwMC0xOTk5OSMlRjAlOUYlODclQkElRjAlOUYlODclQjglMjAlRTclQkUlOEUlRTUlOUIlQkQtJUU5JTk4JUJGJUU0VCVBQy0wMS0lRjAlOUYlOTMlQjY=";
        let res = parse_v2ray_base64(sample).unwrap();
        for out in &res {
            println!("Parsed Tag: {}", out.tag);
        }
        assert!(!res.is_empty());
    }
}
