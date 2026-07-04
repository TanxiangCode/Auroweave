/// V2Ray / Base64 链接列表解析器 (vmess://, vless://, ss://, trojan://)
/// 作者: TanXiang
use super::ParsedOutbound;
use crate::error::AppError;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::json;
use url::Url;

pub fn parse_v2ray_base64(content: &str) -> Result<Vec<ParsedOutbound>, AppError> {
    let decoded = match STANDARD.decode(content.trim().replace("\r\n", "").replace('\n', "")) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(_) => content.to_string(), // 不是合法 base64 的话，当做多行明文 URI 处理
    };

    let lines = decoded.lines();
    let mut outbounds = Vec::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }

        if let Some(parsed) = parse_single_uri(line) {
            outbounds.push(parsed);
        }
    }

    Ok(outbounds)
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
    } else {
        None
    }
}

fn parse_vmess_uri(uri: &str) -> Option<ParsedOutbound> {
    let b64_str = uri.strip_prefix("vmess://")?;
    let decoded_bytes = STANDARD.decode(b64_str).ok()?;
    let json_str = String::from_utf8_lossy(&decoded_bytes);
    let v: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    let name = v.get("ps")?.as_str()?.to_string();
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
    let name = parsed_url.fragment().unwrap_or("Shadowsocks").to_string();
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let user_info = parsed_url.username();

    Some(ParsedOutbound {
        tag: name.clone(),
        r#type: "shadowsocks".to_string(),
        server: Some(server.clone()),
        server_port: Some(port),
        raw_json: json!({
            "type": "shadowsocks",
            "tag": name,
            "server": server,
            "server_port": port,
            "method": "aes-256-gcm",
            "password": user_info
        }),
    })
}

fn parse_trojan_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let name = parsed_url.fragment().unwrap_or("Trojan").to_string();
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let password = parsed_url.username();

    Some(ParsedOutbound {
        tag: name.clone(),
        r#type: "trojan".to_string(),
        server: Some(server.clone()),
        server_port: Some(port),
        raw_json: json!({
            "type": "trojan",
            "tag": name,
            "server": server,
            "server_port": port,
            "password": password
        }),
    })
}

fn parse_vless_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let name = parsed_url.fragment().unwrap_or("VLESS").to_string();
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let uuid = parsed_url.username();

    Some(ParsedOutbound {
        tag: name.clone(),
        r#type: "vless".to_string(),
        server: Some(server.clone()),
        server_port: Some(port),
        raw_json: json!({
            "type": "vless",
            "tag": name,
            "server": server,
            "server_port": port,
            "uuid": uuid
        }),
    })
}
