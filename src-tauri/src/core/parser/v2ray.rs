/// V2Ray / Base64 / URI 列表解析器
/// 支持: vmess://, vless://, ss://, trojan://, hysteria2://, hy2://, anytls://, tuic://
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
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || *c == '+'
                || *c == '/'
                || *c == '-'
                || *c == '_'
                || *c == '='
        })
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
        if let Ok(s) = String::from_utf8(bytes) {
            return Some(s);
        }
    }
    if let Ok(bytes) = URL_SAFE.decode(&padded) {
        if let Ok(s) = String::from_utf8(bytes) {
            return Some(s);
        }
    }
    if let Ok(bytes) = STANDARD_NO_PAD.decode(&sanitized) {
        if let Ok(s) = String::from_utf8(bytes) {
            return Some(s);
        }
    }
    if let Ok(bytes) = URL_SAFE_NO_PAD.decode(&sanitized) {
        if let Ok(s) = String::from_utf8(bytes) {
            return Some(s);
        }
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
    } else if uri.starts_with("tuic://") {
        parse_tuic_uri(uri)
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
        STANDARD
            .decode(b64_str)
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
    })?;

    let v: serde_json::Value = serde_json::from_str(&decoded_bytes).ok()?;

    let name = v
        .get("ps")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("VMess")
        .to_string();

    let server = v
        .get("add")
        .or_else(|| v.get("host"))
        .and_then(|s| s.as_str())?
        .to_string();

    // 兼容数字或字符串格式的 port（校验 1..=65535，拒绝超范围而非静默截断）
    let port = match v.get("port") {
        Some(serde_json::Value::Number(n)) => n.as_u64().and_then(|p| {
            if (1..=65535).contains(&p) {
                Some(p as u16)
            } else {
                None
            }
        }),
        Some(serde_json::Value::String(s)) => s.trim().parse::<u16>().ok().filter(|p| *p >= 1),
        _ => None,
    }?;

    let uuid = v.get("id").and_then(|s| s.as_str())?.to_string();
    let alter_id = match v.get("aid") {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.parse::<u64>().unwrap_or(0),
        _ => 0,
    };
    let security_raw = v
        .get("scy")
        .or_else(|| v.get("cipher"))
        .and_then(|s| s.as_str())
        .unwrap_or("auto");
    // vmess security 白名单（sing-box outbound/vmess.md 全集）：
    // 非法值（aes-128-cfb、chacha20、rc4 等存量生态常见值）会被内核在出站初始化
    // 阶段拒载整份配置，而非仅该节点失败——统一回退 auto
    const VMESS_SECURITY: &[&str] = &[
        "auto",
        "none",
        "zero",
        "aes-128-gcm",
        "chacha20-poly1305",
        "aes-128-ctr",
    ];
    let security = if VMESS_SECURITY.contains(&security_raw) {
        security_raw
    } else {
        log::warn!(
            "[parser] vmess 节点 [{}] 的非法加密方式 {} 回退为 auto",
            name,
            security_raw
        );
        "auto"
    };

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
    let tls = v
        .get("tls")
        .and_then(|s| s.as_str())
        .map(|s| s == "tls" || s == "1")
        .unwrap_or(false);
    let sni = v
        .get("sni")
        .and_then(|s| s.as_str())
        .or_else(|| v.get("host").and_then(|s| s.as_str()));

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
        let raw_path = v.get("path").and_then(|s| s.as_str()).unwrap_or("/");
        let host = v.get("host").and_then(|s| s.as_str()).unwrap_or("");
        // 拆出 "?ed=2048" 早期数据后缀（v2rayN 惯例，见 shared/v2ray-transport.md：
        // 与 Xray 服务端兼容需指定 early_data_header_name 为 Sec-WebSocket-Protocol）
        let (path, max_early) = match raw_path.rsplit_once("?ed=") {
            Some((p, n)) => (p.to_string(), n.parse::<u32>().unwrap_or(0)),
            None => (raw_path.to_string(), 0),
        };
        let mut transport = json!({
            "type": "ws",
            "path": path
        });
        if !host.is_empty() {
            transport["headers"] = json!({ "Host": host });
        }
        if max_early > 0 {
            transport["max_early_data"] = json!(max_early);
            transport["early_data_header_name"] = json!("Sec-WebSocket-Protocol");
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
        Some((b, t)) => (
            b,
            urlencoding::decode(t)
                .unwrap_or_else(|_| t.into())
                .to_string(),
        ),
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

/// 辅助解析 host:port（端口必须落在 1..=65535，超范围拒绝而非截断）
fn parse_host_port(s: &str) -> Option<(String, u16)> {
    let s = s.split('?').next().unwrap_or(s); // 去除 query
    let s = s.split('/').next().unwrap_or(s);
    if let Some((h, p)) = s.rsplit_once(':') {
        let port = p.parse::<u16>().ok().filter(|port| *port >= 1)?;
        let host = h.trim_start_matches('[').trim_end_matches(']').to_string();
        Some((host, port))
    } else {
        None
    }
}

/// 从 URL 提取完整凭据并 percent-decode。
/// `Url::username()/password()` 返回编码原文，密码含 %40 等编码字符时须还原；
/// hy2/anytls 等协议的 `user:pass@` 形式按官方语义整体作为密码（见
/// outbound/hysteria2.md：userpass 认证即 username:password 组合）。
fn extract_credentials(parsed_url: &Url) -> String {
    let mut cred = urlencoding::decode(parsed_url.username())
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| parsed_url.username().to_string());
    if let Some(p) = parsed_url.password() {
        let decoded_p = urlencoding::decode(p)
            .map(|c| c.into_owned())
            .unwrap_or_else(|_| p.to_string());
        cred.push(':');
        cred.push_str(&decoded_p);
    }
    cred
}

/// VLESS 链接解析器
fn parse_vless_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "VLESS");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;
    let uuid = urlencoding::decode(parsed_url.username())
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| parsed_url.username().to_string());

    let mut sni = None;
    let mut security = None;
    let mut pbk = None;
    let mut sid = None;
    let mut flow = None;
    let mut net_type = None;
    let mut path = None;
    let mut service_name = None;
    let mut host = None;
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
            "host" => host = Some(v.to_string()),
            "serviceName" => service_name = Some(v.to_string()),
            "fp" | "fingerprint" => fp = Some(v.to_string()),
            "insecure" | "allowInsecure" => insecure = v == "1" || v == "true",
            "alpn" => {
                alpn = Some(
                    v.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                )
            }
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

    // sing-box 1.14 仅支持 xtls-rprx-vision；旧 URI 的 flow 不能透传，
    // 否则会让整份配置在 check/run 阶段被内核拒绝。
    if flow.as_deref().map(str::trim) == Some("xtls-rprx-vision") {
        raw_json["flow"] = json!("xtls-rprx-vision");
    }

    // 处理 TLS / Reality
    let sec = security.as_deref().unwrap_or("none");
    if sec == "reality" {
        // Reality 必须有 public_key (pbk)，缺失则生成必坏节点，直接拒绝解析
        let pbk_val = pbk.as_deref().map(str::trim).filter(|s| !s.is_empty())?;
        let mut reality_obj = json!({
            "enabled": true,
            "reality": {
                "enabled": true,
                "public_key": pbk_val,
                "short_id": sid.clone().unwrap_or_default()
            }
        });
        if let Some(s) = sni {
            reality_obj["server_name"] = json!(s);
        }
        let fingerprint = fp
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("chrome");
        reality_obj["utls"] = json!({
            "enabled": true,
            "fingerprint": fingerprint
        });
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
            let raw_path = path.clone().unwrap_or_else(|| "/".to_string());
            // 拆出 "?ed=2048" 早期数据后缀（与 vmess 侧一致，Xray 兼容）
            let (pure_path, max_early) = match raw_path.rsplit_once("?ed=") {
                Some((p, n)) => (p.to_string(), n.parse::<u32>().unwrap_or(0)),
                None => (raw_path, 0),
            };
            let mut transport = json!({
                "type": "ws",
                "path": pure_path
            });
            // ws 传输的 host 参数写入 Host 头（CDN 场景下与 path 同样关键）
            if let Some(h) = host.as_deref() {
                if !h.is_empty() {
                    transport["headers"] = json!({ "Host": h });
                }
            }
            if max_early > 0 {
                transport["max_early_data"] = json!(max_early);
                transport["early_data_header_name"] = json!("Sec-WebSocket-Protocol");
            }
            raw_json["transport"] = transport;
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
    // percent-decode 凭据；hy2 userpass（user:pass@）按官方语义整体作为密码
    let password = extract_credentials(&parsed_url);

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
            "alpn" => {
                alpn = Some(
                    v.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                )
            }
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
    // percent-decode 凭据；hy2 userpass（user:pass@）按官方语义整体作为密码
    let password = extract_credentials(&parsed_url);

    let mut sni = None;
    let mut insecure = false;
    let mut obfs_type = None;
    let mut obfs_pass = None;
    // 端口跳跃：mport=2080:3000 或 mport=2080,3001（Hysteria2 官方 URI 生态惯例）
    // sing-box 对应字段为 server_ports 列表，与 server_port 互斥（outbound/hysteria2.md）
    let mut mport: Option<Vec<String>> = None;
    // 1.14.0 新参数：端口跳跃随机化区间 / BBR 档位 / Chrome QUIC 指纹伪装关闭
    let mut hop_interval: Option<String> = None;
    let mut hop_interval_max: Option<String> = None;
    let mut bbr_profile: Option<String> = None;
    let mut disable_chrome_parrot = false;

    for (k, v) in parsed_url.query_pairs() {
        match k.as_ref() {
            "sni" => sni = Some(v.to_string()),
            "insecure" => insecure = v == "1" || v == "true",
            "obfs" => obfs_type = Some(v.to_string()),
            "mport" => {
                let list: Vec<String> = v
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !list.is_empty() {
                    mport = Some(list);
                }
            }
            "hop-interval" | "hop_interval" => hop_interval = Some(v.to_string()),
            "hop-interval-max" | "hop_interval_max" => hop_interval_max = Some(v.to_string()),
            "bbr-profile" | "bbr_profile" => bbr_profile = Some(v.to_string()),
            // 兼容刚需：1.14 默认伪装 Chrome QUIC 握手，Ed25519 证书服务器握手会失败
            "disable-chrome-parrot" | "disable_chrome_parrot" => {
                disable_chrome_parrot = v == "1" || v == "true"
            }
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
    // 端口跳跃生效时移除 server_port（两者互斥，同时存在会拒载）
    if let Some(ports) = mport {
        raw_json["server_ports"] = json!(ports);
        raw_json.as_object_mut().unwrap().remove("server_port");
        // 端口跳跃间隔（默认 30s）；随机化上限仅在显式提供时写入
        if let Some(hi) = hop_interval {
            if !hi.is_empty() {
                raw_json["hop_interval"] = json!(hi);
            }
        }
        if let Some(him) = hop_interval_max {
            if !him.is_empty() {
                raw_json["hop_interval_max"] = json!(him);
            }
        }
    }
    // BBR 拥塞控制档位白名单（outbound/hysteria2.md：conservative/standard/aggressive）
    if let Some(bp) = bbr_profile {
        const BBR_PROFILES: &[&str] = &["conservative", "standard", "aggressive"];
        if BBR_PROFILES.contains(&bp.as_str()) {
            raw_json["bbr_profile"] = json!(bp);
        } else {
            log::warn!("[parser] hy2 节点 [{}] 的非法 BBR 档位 {} 已丢弃", tag, bp);
        }
    }
    if disable_chrome_parrot {
        raw_json["disable_chrome_parrot"] = json!(true);
    }

    // hy2 始终基于 TLS：无条件启用 tls 块，sni 缺省回退 server 地址
    let sni_name = sni.clone().unwrap_or_else(|| server.clone());
    let mut tls = json!({ "enabled": true, "server_name": sni_name });
    if insecure {
        tls["insecure"] = json!(true);
    }
    raw_json["tls"] = tls;

    if let Some(o_type) = obfs_type {
        let mut obfs = json!({ "type": o_type });
        if let Some(p) = obfs_pass {
            obfs["password"] = json!(p);
        }
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
    // percent-decode 凭据；hy2 userpass（user:pass@）按官方语义整体作为密码
    let password = extract_credentials(&parsed_url);

    let mut sni = None;
    let mut fp = "chrome".to_string();
    let mut alpn = vec!["h2".to_string(), "http/1.1".to_string()];
    // 默认不跳过证书校验（与 clash.rs 语义一致），仅显式 insecure=1/true 时开启
    let mut insecure = false;

    for (k, v) in parsed_url.query_pairs() {
        match k.as_ref() {
            "sni" | "peer" | "serverName" => sni = Some(v.to_string()),
            "insecure" | "allowInsecure" | "skip-cert-verify" => insecure = v == "1" || v == "true",
            "fp" | "fingerprint" => fp = v.to_string(),
            "alpn" => {
                let list: Vec<String> = v
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
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

/// TUIC 链接解析器 (sing-box 原生支持 type: "tuic")
/// URI 形如 tuic://uuid:password@host:port?sni=...&alpn=h3&congestion_control=bbr#tag
/// 对应 docs/sing-box_docs/outbound/tuic.md：uuid/password 必填、tls 必填
fn parse_tuic_uri(uri: &str) -> Option<ParsedOutbound> {
    let parsed_url = Url::parse(uri).ok()?;
    let tag = extract_tag(&parsed_url, "TUIC");
    let server = parsed_url.host_str()?.to_string();
    let port = parsed_url.port()?;

    // tuic://uuid:password@ 形式：username 是 uuid，password 是密码
    let uuid = urlencoding::decode(parsed_url.username())
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| parsed_url.username().to_string());
    if uuid.is_empty() {
        return None;
    }
    let password = parsed_url
        .password()
        .map(|p| {
            urlencoding::decode(p)
                .map(|c| c.into_owned())
                .unwrap_or_else(|_| p.to_string())
        })
        .unwrap_or_default();

    let mut sni = None;
    let mut alpn: Option<Vec<String>> = None;
    let mut congestion_control = None;
    let mut udp_relay_mode = None;
    let mut allow_insecure = false;

    for (k, v) in parsed_url.query_pairs() {
        match k.as_ref() {
            "sni" | "peer" | "serverName" => sni = Some(v.to_string()),
            "alpn" => {
                let list: Vec<String> = v
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !list.is_empty() {
                    alpn = Some(list);
                }
            }
            "congestion_control" => congestion_control = Some(v.to_string()),
            "udp_relay_mode" => udp_relay_mode = Some(v.to_string()),
            "allow_insecure" | "allowInsecure" | "insecure" => {
                allow_insecure = v == "1" || v == "true"
            }
            _ => {}
        }
    }

    // congestion_control 白名单（outbound/tuic.md：cubic/new_reno/bbr），非法值丢弃用内核默认
    const TUIC_CC: &[&str] = &["cubic", "new_reno", "bbr"];
    if let Some(cc) = congestion_control.as_deref() {
        if !TUIC_CC.contains(&cc) {
            log::warn!(
                "[parser] TUIC 节点 [{}] 的非法拥塞控制算法 {} 已丢弃",
                tag,
                cc
            );
            congestion_control = None;
        }
    }

    // TUIC 始终基于 QUIC+TLS：无条件启用 tls 块，缺省 alpn 为 h3（QUIC 场景标准）
    let mut tls = json!({
        "enabled": true,
        "alpn": alpn.unwrap_or_else(|| vec!["h3".to_string()])
    });
    if let Some(s) = sni {
        tls["server_name"] = json!(s);
    } else {
        tls["server_name"] = json!(server);
    }
    if allow_insecure {
        tls["insecure"] = json!(true);
    }

    let mut raw_json = json!({
        "type": "tuic",
        "tag": tag,
        "server": server,
        "server_port": port,
        "uuid": uuid,
        "tls": tls
    });
    if !password.is_empty() {
        raw_json["password"] = json!(password);
    }
    if let Some(cc) = congestion_control {
        raw_json["congestion_control"] = json!(cc);
    }
    if let Some(mode) = udp_relay_mode {
        // udp_relay_mode 白名单（native/quic），非法值丢弃
        if mode == "native" || mode == "quic" {
            raw_json["udp_relay_mode"] = json!(mode);
        }
    }

    Some(ParsedOutbound {
        tag,
        r#type: "tuic".to_string(),
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
        let uri = format!(
            "ss://{}@1.2.3.4:8388#%F0%9F%87%BA%F0%9F%87%B8%20%E7%BE%8E%E5%9B%BD",
            user_info
        );
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
        assert_eq!(
            parsed.raw_json.get("type").and_then(|t| t.as_str()),
            Some("anytls")
        );
        assert_eq!(
            parsed.raw_json.get("password").and_then(|p| p.as_str()),
            Some("bd9410fb-d829-4827-b3ad-70039f228b5f")
        );

        let tls = parsed.raw_json.get("tls").expect("应该包含 tls 配置");
        assert_eq!(
            tls.get("server_name").and_then(|s| s.as_str()),
            Some("sg-sjy.9999231.xyz")
        );

        let utls = tls.get("utls").expect("应该包含 utls 配置");
        assert_eq!(
            utls.get("fingerprint").and_then(|s| s.as_str()),
            Some("chrome")
        );

        let alpn = tls
            .get("alpn")
            .and_then(|a| a.as_array())
            .expect("应该包含 alpn 配置");
        assert_eq!(alpn.len(), 2);
    }

    #[test]
    fn test_parse_anytls_default_insecure_false() {
        // 不带 insecure 参数时默认不跳过证书校验（原为默认 true，存在 MITM 风险）
        let uri = "anytls://pw@zf-tw2.9999231.xyz:1023/#%E8%8A%82%E7%82%B9";
        let parsed = parse_anytls_uri(uri).expect("anytls 应该解析成功");
        assert_eq!(parsed.raw_json["tls"]["insecure"], false);
    }

    #[test]
    fn test_parse_tuic_uri() {
        let uri = "tuic://2DD61D93-75D8-4DA4-AC0E-6AECE7EAC365:hello@1.2.3.4:443?congestion_control=bbr&alpn=h3&sni=example.com&udp_relay_mode=native#%F0%9F%87%B8%F0%9F%87%AC%20TUIC-001";
        let parsed = parse_tuic_uri(uri).expect("tuic 应该解析成功");
        assert_eq!(parsed.tag, "🇸🇬 TUIC-001");
        assert_eq!(parsed.r#type, "tuic");
        assert_eq!(parsed.server_port, Some(443));
        assert_eq!(
            parsed.raw_json["uuid"],
            "2DD61D93-75D8-4DA4-AC0E-6AECE7EAC365"
        );
        assert_eq!(parsed.raw_json["password"], "hello");
        assert_eq!(parsed.raw_json["congestion_control"], "bbr");
        assert_eq!(parsed.raw_json["udp_relay_mode"], "native");
        assert_eq!(parsed.raw_json["tls"]["server_name"], "example.com");
        assert_eq!(parsed.raw_json["tls"]["alpn"][0], "h3");
    }

    #[test]
    fn test_parse_tuic_invalid_congestion_control_dropped() {
        // 非法拥塞控制算法不应透传给内核（会拒载），须被白名单过滤
        let uri = "tuic://uuid@1.2.3.4:443?congestion_control=bogus#t";
        let parsed = parse_tuic_uri(uri).expect("tuic 应该解析成功");
        assert!(parsed.raw_json.get("congestion_control").is_none());
        // 无 password 字段时不生成空串
        assert!(parsed.raw_json.get("password").is_none());
    }

    #[test]
    fn test_parse_hy2_port_hopping_and_v14_params() {
        // mport 端口跳跃 + 1.14.0 新参数（hop_interval/hop_interval_max/bbr_profile/disable_chrome_parrot）
        let uri = "hy2://pw@h2.example.com:443?mport=2080:3000&hop-interval=30s&hop-interval-max=60s&bbr-profile=aggressive&disable-chrome-parrot=1&sni=h2.example.com#hy2-hop";
        let parsed = parse_hysteria2_uri(uri).expect("hy2 应该解析成功");
        let raw = &parsed.raw_json;
        assert_eq!(raw["server_ports"][0], "2080:3000");
        assert!(
            raw.get("server_port").is_none(),
            "server_port 与 server_ports 互斥必须移除"
        );
        assert_eq!(raw["hop_interval"], "30s");
        assert_eq!(raw["hop_interval_max"], "60s");
        assert_eq!(raw["bbr_profile"], "aggressive");
        assert_eq!(raw["disable_chrome_parrot"], true);
        assert_eq!(raw["tls"]["server_name"], "h2.example.com");
    }

    #[test]
    fn test_parse_hy2_invalid_bbr_profile_dropped() {
        // 非法 BBR 档位不透传（拒载防护）
        let uri = "hy2://pw@h2.example.com:443?bbr-profile=bogus#t";
        let parsed = parse_hysteria2_uri(uri).expect("hy2 应该解析成功");
        assert!(parsed.raw_json.get("bbr_profile").is_none());
        // 非法 disable_chrome_parrot 值不生效
        let uri2 = "hy2://pw@h2.example.com:443?disable-chrome-parrot=yes#t";
        let parsed2 = parse_hysteria2_uri(uri2).expect("hy2 应该解析成功");
        assert!(parsed2.raw_json.get("disable_chrome_parrot").is_none());
    }

    #[test]
    fn test_parse_hy2_tls_always_present() {
        // 无 sni 参数时 tls 块也必须存在，server_name 回退为 server 地址
        let uri = "hy2://pw@hy2.example.com:443/#hy2-%E8%8A%82%E7%82%B9";
        let parsed = parse_hysteria2_uri(uri).expect("hy2 应该解析成功");
        assert_eq!(parsed.raw_json["tls"]["enabled"], true);
        assert_eq!(parsed.raw_json["tls"]["server_name"], "hy2.example.com");

        // insecure=1 时写入 insecure
        let uri2 = "hy2://pw@hy2.example.com:443/?insecure=1&sni=s.example.com#hy2-2";
        let parsed2 = parse_hysteria2_uri(uri2).expect("hy2 应该解析成功");
        assert_eq!(parsed2.raw_json["tls"]["server_name"], "s.example.com");
        assert_eq!(parsed2.raw_json["tls"]["insecure"], true);
    }

    #[test]
    fn test_parse_vless_flow_whitelist() {
        let vision = "vless://uuid-x@v.example.com:443/?flow=xtls-rprx-vision#vision";
        assert_eq!(
            parse_vless_uri(vision).unwrap().raw_json["flow"],
            "xtls-rprx-vision"
        );

        for flow in ["xtls-rprx-direct", "XTLS-RPRX-VISION", " unknown "] {
            let uri = format!("vless://uuid-x@v.example.com:443/?flow={}#legacy", flow);
            let parsed = parse_vless_uri(&uri).expect("URI 其它字段合法时应保留节点");
            assert!(
                parsed.raw_json.get("flow").is_none(),
                "非法 flow 不得透传: {}",
                flow
            );
        }
    }

    #[test]
    fn test_parse_vless_reality_defaults_chrome_utls() {
        let uri = "vless://uuid-x@v.example.com:443/?security=reality&pbk=public-key&sni=s.example.com#reality";
        let parsed = parse_vless_uri(uri).expect("Reality 节点应解析");
        assert_eq!(parsed.raw_json["tls"]["utls"]["fingerprint"], "chrome");
    }

    #[test]
    fn test_parse_vless_reality_missing_pbk_rejected() {
        // security=reality 但缺 pbk：生成必坏节点，必须拒绝解析（返回 None 跳过该行）
        let uri = "vless://uuid-x@v.example.com:443/?security=reality&sni=s.example.com#reality-%E8%8A%82%E7%82%B9";
        assert!(
            parse_vless_uri(uri).is_none(),
            "缺 pbk 的 reality 节点应被拒绝"
        );
    }

    #[test]
    fn test_parse_vless_ws_host_header() {
        // ws 传输必须把 host 参数写入 transport.headers.Host
        let uri = "vless://uuid-x@v.example.com:443/?type=ws&path=/ws&host=cdn.example.com#ws-%E8%8A%82%E7%82%B9";
        let parsed = parse_vless_uri(uri).expect("vless ws 应该解析成功");
        assert_eq!(parsed.raw_json["transport"]["type"], "ws");
        assert_eq!(
            parsed.raw_json["transport"]["headers"]["Host"],
            "cdn.example.com"
        );
    }

    #[test]
    fn test_parse_vmess_port_out_of_range_rejected() {
        // 端口 70000 超范围应拒绝该节点而非静默截断为 4464
        let vmess_json = json!({
            "v": "2",
            "ps": "超大端口节点",
            "add": "1.2.3.4",
            "port": 70000,
            "id": "a0000000-0000-0000-0000-000000000001",
            "net": "tcp"
        });
        let b64 = STANDARD.encode(vmess_json.to_string());
        let uri = format!("vmess://{}", b64);
        assert!(parse_vmess_uri(&uri).is_none(), "超范围端口应拒绝解析");
    }
}
