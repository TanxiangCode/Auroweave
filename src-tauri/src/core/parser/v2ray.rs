/// V2Ray / Base64 / URI 列表解析器
/// 支持: vmess://, vless://, ss://, trojan://, hysteria2://, hy2://, anytls://
/// 作者: TanXiang
use super::ParsedOutbound;
use crate::error::AppError;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use serde_json::json;
use url::Url;

/// 解析包含多行节点 URI 或 Base64 编码的订阅内容
pub fn parse_v2ray_base64(content: &str) -> Result<Vec<ParsedOutbound>, AppError> {
    let clean = content.trim().replace("\r\n", "\n").replace('\r', "");
    if clean.is_empty() {
        return Ok(Vec::new());
    }

    // 第一级嗅探：检查是否直接是多行明文 URI 列表
    let mut outbounds = parse_lines_to_outbounds(&clean);
    if !outbounds.is_empty() {
        return Ok(outbounds);
    }

    // 第二级嗅探：整串内容可能是 Base64 编码
    if let Some(decoded) = flexible_base64_decode(&clean) {
        outbounds = parse_lines_to_outbounds(&decoded);
        if !outbounds.is_empty() {
            return Ok(outbounds);
        }
    }

    Ok(Vec::new())
}

/// 逐行解析文本中的节点 URI
fn parse_lines_to_outbounds(text: &str) -> Vec<ParsedOutbound> {
    let mut outbounds = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(parsed) = parse_single_uri(line) {
            outbounds.push(parsed);
        }
    }
    outbounds
}

/// 兼容多种 Base64 编码变体与补齐
pub fn flexible_base64_decode(input: &str) -> Option<String> {
    let sanitized: String = input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '-' || *c == '_' || *c == '=')
        .collect();
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

/// 解析单条 URI 链接
fn parse_single_uri(uri: &str) -> Option<ParsedOutbound> {
    let uri = uri.trim();
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

/// VMess 链接解析器
fn parse_vmess_uri(uri: &str) -> Option<ParsedOutbound> {
    let b64_str = uri.strip_prefix("vmess://")?;
    let decoded_bytes = flexible_base64_decode(b64_str).or_else(|| {
        STANDARD.decode(b64_str).ok().and_then(|b| String::from_utf8(b).ok())
    })?;

    let v: serde_json::Value = serde_json::from_str(&decoded_bytes).ok()?;

    let name = v.get("ps")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("VMess")
        .to_string();

    let server = v.get("add")
        .or_else(|| v.get("host"))
        .and_then(|s| s.as_str())?
        .to_string();

    // 兼容数字或字符串格式的 port
    let port = match v.get("port") {
        Some(serde_json::Value::Number(n)) => n.as_u64().map(|p| p as u16),
        Some(serde_json::Value::String(s)) => s.parse::<u16>().ok(),
        _ => None,
    }?;

    let uuid = v.get("id").and_then(|s| s.as_str())?.to_string();
    let alter_id = match v.get("aid") {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.parse::<u64>().unwrap_or(0),
        _ => 0,
    };
    let security = v.get("scy").or_else(|| v.get("cipher")).and_then(|s| s.as_str()).unwrap_or("auto");

    let mut raw_json = json!({
        "type": "vmess",
        "tag": name,
        "server": server,
        "server_port": port,
        "uuid": uuid,
        "security": security,
        "alter_id": alter_id
    });

    let net = v.get("net").and_then(|s| s.as_str()).unwrap_or("tcp");
    let tls = v.get("tls").and_then(|s| s.as_str()).map(|s| s == "tls" || s == "1").unwrap_or(false);
    let sni = v.get("sni").and_then(|s| s.as_str()).or_else(|| v.get("host").and_then(|s| s.as_str()));

    if tls {
        let mut tls_obj = json!({ "enabled": true });
        if let Some(s) = sni {
            if !s.is_empty() {
                tls_obj["server_name"] = json!(s);
            }
        }
        raw_json["tls"] = tls_obj;
    }

    if net == "ws" {
        let path = v.get("path").and_then(|s| s.as_str()).unwrap_or("/");
        let host = v.get("host").and_then(|s| s.as_str()).unwrap_or("");
        let mut transport = json!({
            "type": "ws",
            "path": path
        });
        if !host.is_empty() {
            transport["headers"] = json!({ "Host": host });
        }
        raw_json["transport"] = transport;
    } else if net == "grpc" {
        let path = v.get("path").and_then(|s| s.as_str()).unwrap_or("");
        raw_json["transport"] = json!({
            "type": "grpc",
            "service_name": path
        });
    }

    Some(ParsedOutbound {
        tag: name,
        r#type: "vmess".to_string(),
        server: Some(server),
        server_port: Some(port),
        raw_json,
    })
}

/// Shadowsocks 链接解析器 (支持 SIP002 标准与 Legacy Base64 格式)
fn parse_ss_uri(uri: &str) -> Option<ParsedOutbound> {
    let raw_part = uri.strip_prefix("ss://")?;
    let (body, tag) = match raw_part.split_once('#') {
        Some((b, t)) => (b, urlencoding::decode(t).unwrap_or_else(|_| t.into()).to_string()),
        None => (raw_part, "Shadowsocks".to_string()),
    };

    // 格式1: SIP002 -> ss://BASE64(method:password)@server:port
    if let Some((user_info_b64, server_part)) = body.split_once('@') {
        let decoded_user = flexible_base64_decode(user_info_b64)
            .or_else(|| String::from_utf8(STANDARD.decode(user_info_b64).ok()?).ok())
            .unwrap_or_else(|| user_info_b64.to_string());

        let (method, password) = match decoded_user.split_once(':') {
            Some((m, p)) => (m.to_string(), p.to_string()),
            None => ("aes-256-gcm".to_string(), decoded_user),
        };

        let (server, port) = parse_host_port(server_part)?;

        return Some(ParsedOutbound {
            tag: tag.clone(),
            r#type: "shadowsocks".to_string(),
            server: Some(server.clone()),
            server_port: Some(port),
            raw_json: json!({
                "type": "shadowsocks",
                "tag": tag,
                "server": server,
                "server_port": port,
                "method": method,
                "password": password
            }),
        });
    }

    // 格式2: Legacy -> ss://BASE64(method:password@server:port)
    if let Some(decoded_full) = flexible_base64_decode(body) {
        if let Some((user_info, server_part)) = decoded_full.split_once('@') {
            let (method, password) = match user_info.split_once(':') {
                Some((m, p)) => (m.to_string(), p.to_string()),
                None => ("aes-256-gcm".to_string(), user_info.to_string()),
            };
            let (server, port) = parse_host_port(server_part)?;

            return Some(ParsedOutbound {
                tag: tag.clone(),
                r#type: "shadowsocks".to_string(),
                server: Some(server.clone()),
                server_port: Some(port),
                raw_json: json!({
                    "type": "shadowsocks",
                    "tag": tag,
                    "server": server,
                    "server_port": port,
                    "method": method,
                    "password": password
                }),
            });
        }
    }

    None
}

/// 辅助解析 host:port
fn parse_host_port(s: &str) -> Option<(String, u16)> {
    let s = s.split('?').next().unwrap_or(s); // 去除 query
    let s = s.split('/').next().unwrap_or(s);
    if let Some((h, p)) = s.rsplit_once(':') {
        let port = p.parse::<u16>().ok()?;
        let host = h.trim_start_matches('[').trim_end_matches(']').to_string();
        Some((host, port))
    } else {
        None
    }
}

/// VLESS 链接解析器
fn parse_vless_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "VLESS");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let uuid = parsed_url.username().to_string();

    let mut sni = None;
    let mut security = None;
    let mut pbk = None;
    let mut sid = None;
    let mut flow = None;
    let mut net_type = None;
    let mut path = None;
    let mut service_name = None;
    let mut fp = None;
    let mut insecure = false;
    let mut alpn: Option<Vec<String>> = None;

    for (k, v) in parsed_url.query_pairs() {
        match k.as_ref() {
            "sni" | "serverName" | "peer" => sni = Some(v.to_string()),
            "security" => security = Some(v.to_string()),
            "pbk" => pbk = Some(v.to_string()),
            "sid" => sid = Some(v.to_string()),
            "flow" => flow = Some(v.to_string()),
            "type" => net_type = Some(v.to_string()),
            "path" => path = Some(v.to_string()),
            "serviceName" => service_name = Some(v.to_string()),
            "fp" | "fingerprint" => fp = Some(v.to_string()),
            "insecure" | "allowInsecure" => insecure = v == "1" || v == "true",
            "alpn" => alpn = Some(v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()),
            _ => {}
        }
    }

    let mut raw_json = json!({
        "type": "vless",
        "tag": tag,
        "server": server,
        "server_port": port,
        "uuid": uuid,
        "packet_encoding": "xudp"
    });

    if let Some(f) = flow {
        if !f.is_empty() {
            raw_json["flow"] = json!(f);
        }
    }

    // 处理 TLS / Reality
    let sec = security.as_deref().unwrap_or("none");
    if sec == "reality" {
        let mut reality_obj = json!({
            "enabled": true,
            "reality": {
                "enabled": true,
                "public_key": pbk.unwrap_or_default(),
                "short_id": sid.unwrap_or_default()
            }
        });
        if let Some(s) = sni {
            reality_obj["server_name"] = json!(s);
        }
        if let Some(f) = fp {
            reality_obj["utls"] = json!({ "enabled": true, "fingerprint": f });
        }
        raw_json["tls"] = reality_obj;
    } else if sec == "tls" || sni.is_some() {
        let mut tls_obj = json!({ "enabled": true });
        if let Some(s) = sni {
            tls_obj["server_name"] = json!(s);
        }
        if insecure {
            tls_obj["insecure"] = json!(true);
        }
        if let Some(f) = fp {
            tls_obj["utls"] = json!({ "enabled": true, "fingerprint": f });
        }
        if let Some(a) = alpn {
            tls_obj["alpn"] = json!(a);
        }
        raw_json["tls"] = tls_obj;
    }

    // 处理 Transport
    if let Some(t) = net_type {
        if t == "ws" {
            raw_json["transport"] = json!({
                "type": "ws",
                "path": path.unwrap_or_else(|| "/".to_string())
            });
        } else if t == "grpc" {
            raw_json["transport"] = json!({
                "type": "grpc",
                "service_name": service_name.or(path).unwrap_or_default()
            });
        }
    }

    Some(ParsedOutbound {
        tag,
        r#type: "vless".to_string(),
        server: Some(server),
        server_port: Some(port),
        raw_json,
    })
}

/// Trojan 链接解析器
fn parse_trojan_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "Trojan");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let password = parsed_url.username().to_string();

    let mut sni = None;
    let mut net_type = None;
    let mut path = None;
    let mut fp = None;
    let mut insecure = false;
    let mut alpn: Option<Vec<String>> = None;

    for (k, v) in parsed_url.query_pairs() {
        match k.as_ref() {
            "sni" | "peer" | "serverName" => sni = Some(v.to_string()),
            "type" => net_type = Some(v.to_string()),
            "path" => path = Some(v.to_string()),
            "fp" | "fingerprint" => fp = Some(v.to_string()),
            "insecure" | "allowInsecure" => insecure = v == "1" || v == "true",
            "alpn" => alpn = Some(v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()),
            _ => {}
        }
    }

    let mut tls_obj = json!({ "enabled": true });
    if let Some(s) = sni {
        tls_obj["server_name"] = json!(s);
    } else {
        tls_obj["server_name"] = json!(server);
    }
    if insecure {
        tls_obj["insecure"] = json!(true);
    }
    if let Some(f) = fp {
        tls_obj["utls"] = json!({ "enabled": true, "fingerprint": f });
    }
    if let Some(a) = alpn {
        tls_obj["alpn"] = json!(a);
    }

    let mut raw_json = json!({
        "type": "trojan",
        "tag": tag,
        "server": server,
        "server_port": port,
        "password": password,
        "tls": tls_obj
    });

    if let Some(t) = net_type {
        if t == "ws" {
            raw_json["transport"] = json!({
                "type": "ws",
                "path": path.unwrap_or_else(|| "/".to_string())
            });
        }
    }

    Some(ParsedOutbound {
        tag,
        r#type: "trojan".to_string(),
        server: Some(server),
        server_port: Some(port),
        raw_json,
    })
}

/// Hysteria2 链接解析器
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
        tag,
        r#type: "hysteria2".to_string(),
        server: Some(server),
        server_port: Some(port),
        raw_json,
    })
}

/// AnyTLS 链接解析器 (sing-box 原生支持 type: "anytls")
fn parse_anytls_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "AnyTLS");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let password = parsed_url.username().to_string();

    let mut sni = None;
    let mut fp = "chrome".to_string();
    let mut alpn = vec!["h2".to_string(), "http/1.1".to_string()];
    let mut insecure = true;

    for (k, v) in parsed_url.query_pairs() {
        match k.as_ref() {
            "sni" | "peer" | "serverName" => sni = Some(v.to_string()),
            "insecure" | "allowInsecure" | "skip-cert-verify" => insecure = v == "1" || v == "true",
            "fp" | "fingerprint" => fp = v.to_string(),
            "alpn" => {
                let list: Vec<String> = v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                if !list.is_empty() {
                    alpn = list;
                }
            }
            _ => {}
        }
    }

    let mut tls = json!({
        "enabled": true,
        "insecure": insecure,
        "utls": {
            "enabled": true,
            "fingerprint": fp
        },
        "alpn": alpn
    });
    if let Some(s) = sni {
        tls["server_name"] = json!(s);
    } else {
        tls["server_name"] = json!(server);
    }

    let raw_json = json!({
        "type": "anytls",
        "tag": tag,
        "server": server,
        "server_port": port,
        "password": password,
        "tls": tls
    });

    Some(ParsedOutbound {
        tag,
        r#type: "anytls".to_string(),
        server: Some(server),
        server_port: Some(port),
        raw_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vmess_number_and_string_port() {
        let vmess_json_num = json!({
            "v": "2",
            "ps": "测试节点-数字端口",
            "add": "1.2.3.4",
            "port": 443,
            "id": "a0000000-0000-0000-0000-000000000001",
            "net": "ws",
            "type": "none",
            "tls": "tls"
        });
        let b64 = STANDARD.encode(vmess_json_num.to_string());
        let uri = format!("vmess://{}", b64);
        let parsed = parse_vmess_uri(&uri);
        assert!(parsed.is_some());
        let p = parsed.unwrap();
        assert_eq!(p.tag, "测试节点-数字端口");
        assert_eq!(p.server_port, Some(443));
    }

    #[test]
    fn test_parse_ss_sip002() {
        let user_info = STANDARD.encode("aes-128-gcm:pass123");
        let uri = format!("ss://{}@1.2.3.4:8388#%F0%9F%87%BA%F0%9F%87%B8%20%E7%BE%8E%E5%9B%BD", user_info);
        let parsed = parse_ss_uri(&uri);
        assert!(parsed.is_some());
        let p = parsed.unwrap();
        assert_eq!(p.server_port, Some(8388));
        assert_eq!(p.raw_json["method"], "aes-128-gcm");
        assert_eq!(p.raw_json["password"], "pass123");
    }

    #[test]
    fn test_parse_anytls_uri() {
        let uri = "anytls://bd9410fb-d829-4827-b3ad-70039f228b5f@zf-tw2.9999231.xyz:1023/?type=tcp&insecure=0&fp=chrome&sni=sg-sjy.9999231.xyz#%F0%9F%87%B8%F0%9F%87%AC%20%E6%96%B0%E5%8A%A0%E5%9D%A1-002";
        let parsed = parse_anytls_uri(uri).expect("anytls 应该解析成功");
        assert_eq!(parsed.tag, "🇸🇬 新加坡-002");
        assert_eq!(parsed.server_port, Some(1023));
        assert_eq!(parsed.r#type, "anytls");
        assert_eq!(parsed.raw_json.get("type").and_then(|t| t.as_str()), Some("anytls"));
        assert_eq!(parsed.raw_json.get("password").and_then(|p| p.as_str()), Some("bd9410fb-d829-4827-b3ad-70039f228b5f"));

        let tls = parsed.raw_json.get("tls").expect("应该包含 tls 配置");
        assert_eq!(tls.get("server_name").and_then(|s| s.as_str()), Some("sg-sjy.9999231.xyz"));

        let utls = tls.get("utls").expect("应该包含 utls 配置");
        assert_eq!(utls.get("fingerprint").and_then(|s| s.as_str()), Some("chrome"));

        let alpn = tls.get("alpn").and_then(|a| a.as_array()).expect("应该包含 alpn 配置");
        assert_eq!(alpn.len(), 2);
    }
}


