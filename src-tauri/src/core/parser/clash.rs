/// Clash / Mihomo YAML 订阅解析器
/// 作者: TanXiang
use super::ParsedOutbound;
use crate::error::AppError;
use serde_json::json;
use serde_yaml::Value as YamlValue;

pub fn parse_clash_yaml(content: &str) -> Result<Vec<ParsedOutbound>, AppError> {
    let yaml: YamlValue = serde_yaml::from_str(content)
        .map_err(|e| AppError::Subscription(format!("Clash YAML 解析失败: {}", e)))?;

    let proxies_val = yaml.get("proxies").or_else(|| yaml.get("Proxy"));
    let proxies_arr = match proxies_val.and_then(|v| v.as_sequence()) {
        Some(arr) => arr,
        None => return Err(AppError::Subscription("Clash 订阅未包含 proxies 节点".to_string())),
    };

    let mut outbounds = Vec::new();
    // 记录首个解析失败代理的名称与原因，供最终错误信息携带（避免逐层吞掉）
    let mut first_failure: Option<String> = None;

    for proxy in proxies_arr {
        if let Some(parsed) = convert_clash_proxy_to_singbox(proxy) {
            outbounds.push(parsed);
        } else if first_failure.is_none() {
            let name = proxy.get("name").and_then(|v| v.as_str()).unwrap_or("<无名>");
            let ptype = proxy.get("type").and_then(|v| v.as_str()).unwrap_or("<无类型>");
            let port_invalid = proxy
                .get("port")
                .and_then(|v| v.as_f64())
                .map(|p| !(1.0..=65535.0).contains(&p))
                .unwrap_or(false);
            first_failure = Some(if port_invalid {
                format!("代理 [{}] (type: {}) 端口非法", name, ptype)
            } else {
                format!("代理 [{}] (type: {}) 缺少必要字段或类型不支持", name, ptype)
            });
        }
    }

    if outbounds.is_empty() {
        let detail = first_failure.unwrap_or_else(|| "未解析出任何节点".to_string());
        return Err(AppError::Subscription(format!(
            "Clash 订阅中未找到有效节点（首个失败原因: {}）",
            detail
        )));
    }

    Ok(outbounds)
}

/// 校验端口范围：YAML 中 port 可能是数字或数字字符串
/// 返回 None 表示端口缺失或超出 1..=65535（拒绝而非静默截断）
fn extract_port(proxy: &YamlValue) -> Option<u16> {
    let raw = proxy.get("port")?;
    let port_u64 = match raw {
        YamlValue::Number(n) => n.as_u64(),
        YamlValue::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }?;
    if (1..=65535).contains(&port_u64) {
        Some(port_u64 as u16)
    } else {
        None
    }
}

/// 从 ws path 的 "?ed=2048" 后缀拆出 early data 参数（v2rayN 生态惯例）：
/// `/ws?ed=2048` → path=`/ws` + max_early_data=2048 + early_data_header_name=Sec-WebSocket-Protocol
/// （sing-box 早期数据默认走 path，与 Xray 服务端兼容必须指定 header 名，见 shared/v2ray-transport.md）
fn split_ws_early_data(path: &str) -> (String, Option<u32>) {
    match path.rsplit_once("?ed=") {
        Some((pure, n)) => {
            let max_early = n.parse::<u32>().unwrap_or(0);
            if max_early > 0 {
                (pure.to_string(), Some(max_early))
            } else {
                (path.to_string(), None)
            }
        }
        None => (path.to_string(), None),
    }
}

/// 构建 Clash YAML 的 ws/grpc 传输层 JSON（vmess/vless/trojan 共用）。
/// 关键字段：ws-opts.headers.Host（CDN 回源必需，历史缺陷丢失导致节点不可用）。
fn build_clash_transport(net: &str, proxy: &YamlValue) -> Option<serde_json::Value> {
    match net {
        "ws" => {
            let ws_opts = proxy.get("ws-opts");
            let raw_path = ws_opts.and_then(|v| v.get("path")).and_then(|v| v.as_str()).unwrap_or("/");
            let (path, max_early) = split_ws_early_data(raw_path);
            let mut t = json!({ "type": "ws", "path": path });
            if let Some(host) = ws_opts
                .and_then(|v| v.get("headers"))
                .and_then(|h| h.get("Host").or_else(|| h.get("host")))
                .and_then(|v| v.as_str())
            {
                if !host.is_empty() {
                    t["headers"] = json!({ "Host": host });
                }
            }
            if let Some(ed) = max_early {
                t["max_early_data"] = json!(ed);
                t["early_data_header_name"] = json!("Sec-WebSocket-Protocol");
            }
            Some(t)
        }
        "grpc" => {
            let service_name = proxy
                .get("grpc-opts")
                .and_then(|v| v.get("grpc-service-name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Some(json!({ "type": "grpc", "service_name": service_name }))
        }
        _ => None,
    }
}

fn convert_clash_proxy_to_singbox(proxy: &YamlValue) -> Option<ParsedOutbound> {
    let name = proxy.get("name")?.as_str()?.to_string();
    let proxy_type = proxy.get("type")?.as_str()?.to_lowercase();
    let server = proxy.get("server")?.as_str()?.to_string();
    let port = extract_port(proxy)?;

    let (singbox_type, raw_json) = match proxy_type.as_str() {
        "ss" | "shadowsocks" => {
            let cipher = proxy.get("cipher")?.as_str()?.to_string();
            let password = proxy.get("password")?.as_str()?.to_string();
            ("shadowsocks".to_string(), json!({
                "type": "shadowsocks",
                "tag": name,
                "server": server,
                "server_port": port,
                "method": cipher,
                "password": password
            }))
        }
        "vmess" => {
            let uuid = proxy.get("uuid")?.as_str()?.to_string();
            // alterId 字符串形式（"64"）兼容：老订阅常见
            let alter_id = match proxy.get("alterId") {
                Some(serde_yaml::Value::Number(n)) => n.as_u64().unwrap_or(0),
                Some(serde_yaml::Value::String(s)) => s.parse::<u64>().unwrap_or(0),
                _ => 0,
            };
            let cipher_raw = proxy.get("cipher").and_then(|v| v.as_str()).unwrap_or("auto");
            // vmess security 白名单（同 v2ray.rs，非法值回退 auto 防整配置拒载）
            const VMESS_SECURITY: &[&str] = &["auto", "none", "zero", "aes-128-gcm", "chacha20-poly1305", "aes-128-ctr"];
            let cipher = if VMESS_SECURITY.contains(&cipher_raw) { cipher_raw } else {
                log::warn!("[parser] Clash vmess 节点 [{}] 的非法加密方式 {} 回退为 auto", name, cipher_raw);
                "auto"
            };
            let mut obj = json!({
                "type": "vmess",
                "tag": name,
                "server": server,
                "server_port": port,
                "uuid": uuid,
                "security": cipher,
                "alter_id": alter_id
            });
            if let Some(tls) = proxy.get("tls").and_then(|v| v.as_bool()) {
                if tls {
                    let sni = proxy.get("servername").or_else(|| proxy.get("sni")).and_then(|v| v.as_str());
                    let skip_verify = proxy.get("skip-cert-verify").and_then(|v| v.as_bool()).unwrap_or(false);
                    let mut tls_obj = json!({ "enabled": true });
                    if let Some(s) = sni { tls_obj["server_name"] = json!(s); }
                    if skip_verify { tls_obj["insecure"] = json!(true); }
                    obj["tls"] = tls_obj;
                }
            }
            if let Some(net) = proxy.get("network").and_then(|v| v.as_str()) {
                if let Some(transport) = build_clash_transport(net, proxy) {
                    obj["transport"] = transport;
                }
            }
            ("vmess".to_string(), obj)
        }
        "vless" => {
            let uuid = proxy.get("uuid")?.as_str()?.to_string();
            let mut obj = json!({
                "type": "vless",
                "tag": name,
                "server": server,
                "server_port": port,
                "uuid": uuid
            });
            if let Some(flow) = proxy.get("flow").and_then(|v| v.as_str()) {
                // vless flow 白名单：sing-box 1.14 仅支持 xtls-rprx-vision，
                // 旧值（xtls-rprx-direct 等）会被服务端拒绝，直接丢弃
                if flow == "xtls-rprx-vision" { obj["flow"] = json!(flow); }
            }
            // vless 同样支持 ws/grpc 传输（历史缺陷：vless 分支完全丢失 network 处理，
            // 带 ws-opts 的 vless 节点转换后裸 TCP 直连，节点必坏）
            if let Some(net) = proxy.get("network").and_then(|v| v.as_str()) {
                if let Some(transport) = build_clash_transport(net, proxy) {
                    obj["transport"] = transport;
                }
            }
            if let Some(reality_opts) = proxy.get("reality-opts") {
                let public_key = reality_opts.get("public-key").and_then(|v| v.as_str()).unwrap_or("");
                let short_id = reality_opts.get("short-id").and_then(|v| v.as_str()).unwrap_or("");
                // reality 的 public_key 是必填项（文档 shared/tls.md：Required），
                // 空串握手必失败——与 v2ray URI 侧对齐，直接拒绝解析该节点
                if public_key.is_empty() {
                    log::warn!("[parser] Clash vless reality 节点 [{}] 缺少 public-key，跳过", name);
                    return None;
                }
                let sni = proxy.get("servername").or_else(|| proxy.get("sni")).and_then(|v| v.as_str());
                let fingerprint = proxy.get("client-fingerprint").and_then(|v| v.as_str()).unwrap_or("chrome");
                let mut reality = json!({
                    "enabled": true,
                    "utls": { "enabled": true, "fingerprint": fingerprint },
                    "reality": {
                        "enabled": true,
                        "public_key": public_key,
                        "short_id": short_id
                    }
                });
                if let Some(s) = sni { reality["server_name"] = json!(s); }
                obj["tls"] = reality;
            } else if proxy.get("tls").and_then(|v| v.as_bool()).unwrap_or(false) {
                let sni = proxy.get("servername").or_else(|| proxy.get("sni")).and_then(|v| v.as_str());
                let skip_verify = proxy.get("skip-cert-verify").and_then(|v| v.as_bool()).unwrap_or(false);
                let fingerprint = proxy.get("client-fingerprint").and_then(|v| v.as_str()).unwrap_or("chrome");
                let mut tls_obj = json!({
                    "enabled": true,
                    "utls": { "enabled": true, "fingerprint": fingerprint }
                });
                if let Some(s) = sni { tls_obj["server_name"] = json!(s); }
                if skip_verify { tls_obj["insecure"] = json!(true); }
                obj["tls"] = tls_obj;
            }
            ("vless".to_string(), obj)
        }
        "trojan" => {
            let password = proxy.get("password")?.as_str()?.to_string();
            let sni = proxy.get("sni").or_else(|| proxy.get("servername")).and_then(|v| v.as_str()).unwrap_or(&server);
            let skip_verify = proxy.get("skip-cert-verify").and_then(|v| v.as_bool()).unwrap_or(false);
            let mut tls_obj = json!({
                "enabled": true,
                "server_name": sni
            });
            if skip_verify { tls_obj["insecure"] = json!(true); }
            let mut obj = json!({
                "type": "trojan",
                "tag": name,
                "server": server,
                "server_port": port,
                "password": password,
                "tls": tls_obj
            });
            if let Some(net) = proxy.get("network").and_then(|v| v.as_str()) {
                if let Some(transport) = build_clash_transport(net, proxy) {
                    obj["transport"] = transport;
                }
            }
            ("trojan".to_string(), obj)
        }
        "hysteria2" | "hy2" => {
            let password = proxy.get("password").and_then(|v| v.as_str()).unwrap_or("");
            let mut obj = json!({
                "type": "hysteria2",
                "tag": name,
                "server": server,
                "server_port": port,
                "password": password
            });
            // hy2 始终基于 TLS：sni 缺省时回退 server 地址，确保 tls 块必定存在
            let sni = proxy.get("sni").or_else(|| proxy.get("servername")).and_then(|v| v.as_str()).unwrap_or(&server);
            let skip_verify = proxy.get("skip-cert-verify").and_then(|v| v.as_bool()).unwrap_or(false);
            let mut tls_obj = json!({ "enabled": true, "server_name": sni });
            if skip_verify { tls_obj["insecure"] = json!(true); }
            obj["tls"] = tls_obj;
            if let Some(obfs) = proxy.get("obfs").and_then(|v| v.as_str()) {
                let obfs_pass = proxy.get("obfs-password").and_then(|v| v.as_str()).unwrap_or("");
                obj["obfs"] = json!({ "type": obfs, "password": obfs_pass });
            }
            ("hysteria2".to_string(), obj)
        }
        "anytls" => {
            let password = proxy.get("password").or_else(|| proxy.get("uuid")).and_then(|v| v.as_str()).unwrap_or("");
            let sni = proxy.get("sni").or_else(|| proxy.get("servername")).and_then(|v| v.as_str());
            let insecure = proxy.get("skip-cert-verify").and_then(|v| v.as_bool()).unwrap_or(false);
            let fingerprint = proxy.get("client-fingerprint").and_then(|v| v.as_str()).unwrap_or("chrome");

            let mut tls_obj = json!({
                "enabled": true,
                "insecure": insecure,
                "utls": {
                    "enabled": true,
                    "fingerprint": fingerprint
                }
            });
            if let Some(s) = sni {
                tls_obj["server_name"] = json!(s);
            } else {
                tls_obj["server_name"] = json!(server);
            }
            if let Some(alpn_seq) = proxy.get("alpn").and_then(|v| v.as_sequence()) {
                let alpn_list: Vec<String> = alpn_seq.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                if !alpn_list.is_empty() {
                    tls_obj["alpn"] = json!(alpn_list);
                }
            } else {
                tls_obj["alpn"] = json!(["h2", "http/1.1"]);
            }

            ("anytls".to_string(), json!({
                "type": "anytls",
                "tag": name,
                "server": server,
                "server_port": port,
                "password": password,
                "tls": tls_obj
            }))
        }
        "http" => {
            let username = proxy.get("username").and_then(|v| v.as_str()).unwrap_or("");
            let password = proxy.get("password").and_then(|v| v.as_str()).unwrap_or("");
            let mut obj = json!({
                "type": "http",
                "tag": name,
                "server": server,
                "server_port": port
            });
            if !username.is_empty() { obj["username"] = json!(username); }
            if !password.is_empty() { obj["password"] = json!(password); }
            ("http".to_string(), obj)
        }
        "socks5" | "socks" => {
            let username = proxy.get("username").and_then(|v| v.as_str()).unwrap_or("");
            let password = proxy.get("password").and_then(|v| v.as_str()).unwrap_or("");
            let mut obj = json!({
                "type": "socks",
                "tag": name,
                "server": server,
                "server_port": port
            });
            if !username.is_empty() { obj["username"] = json!(username); }
            if !password.is_empty() { obj["password"] = json!(password); }
            ("socks".to_string(), obj)
        }
        _ => return None,
    };

    Some(ParsedOutbound {
        tag: name,
        r#type: singbox_type,
        server: Some(server),
        server_port: Some(port),
        raw_json,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clash_anytls() {
        let yaml = r#"
proxies:
  - { name: '🇹🇼 台湾-住宅家宽-001', type: anytls, server: zf-tw1.9999231.xyz, port: 500, password: bd9410fb-d829-4827-b3ad-70039f228b5f, client-fingerprint: chrome, udp: true, alpn: [h2, http/1.1], sni: tw01.9999231.xyz, skip-cert-verify: false }
"#;
        let outbounds = parse_clash_yaml(yaml).unwrap();
        assert_eq!(outbounds.len(), 1);
        let node = &outbounds[0];
        assert_eq!(node.tag, "🇹🇼 台湾-住宅家宽-001");
        assert_eq!(node.r#type, "anytls");
        assert_eq!(node.server, Some("zf-tw1.9999231.xyz".to_string()));
        assert_eq!(node.server_port, Some(500));
        assert_eq!(node.raw_json["type"], "anytls");
        assert_eq!(node.raw_json["password"], "bd9410fb-d829-4827-b3ad-70039f228b5f");
        assert_eq!(node.raw_json["tls"]["enabled"], true);
        assert_eq!(node.raw_json["tls"]["server_name"], "tw01.9999231.xyz");
        assert_eq!(node.raw_json["tls"]["utls"]["fingerprint"], "chrome");
    }

    #[test]
    fn test_parse_clash_hy2_tls_fallback_and_insecure() {
        // 无 sni 时 tls 块仍必须存在，server_name 回退为 server 地址
        let yaml_no_sni = r#"
proxies:
  - { name: 'hy2-无sni', type: hy2, server: hy2.example.com, port: 443, password: pw }
"#;
        let outbounds = parse_clash_yaml(yaml_no_sni).unwrap();
        assert_eq!(outbounds.len(), 1);
        let tls = &outbounds[0].raw_json["tls"];
        assert_eq!(tls["enabled"], true);
        assert_eq!(tls["server_name"], "hy2.example.com");

        // skip-cert-verify: true 应写入 tls.insecure
        let yaml_insecure = r#"
proxies:
  - { name: 'hy2-跳过校验', type: hysteria2, server: hy2.example.com, port: 443, password: pw, sni: sni.example.com, skip-cert-verify: true }
"#;
        let outbounds = parse_clash_yaml(yaml_insecure).unwrap();
        assert_eq!(outbounds[0].raw_json["tls"]["insecure"], true);
        assert_eq!(outbounds[0].raw_json["tls"]["server_name"], "sni.example.com");
    }

    #[test]
    fn test_parse_clash_port_out_of_range_rejected() {
        // 端口 70000 超出 1..=65535 应拒绝该节点（返回 None），不能静默截断
        let yaml_bad_port = r#"
proxies:
  - { name: 'bad-port', type: ss, server: a.example.com, port: 70000, cipher: aes-256-gcm, password: pw }
"#;
        let result = parse_clash_yaml(yaml_bad_port);
        assert!(result.is_err(), "非法端口订阅应整体返回错误");
    }

    #[test]
    fn test_parse_clash_vless_skip_cert_verify() {
        let yaml = r#"
proxies:
  - { name: 'vless-跳过校验', type: vless, server: v.example.com, port: 443, uuid: uuid-x, tls: true, skip-cert-verify: true }
  - { name: 'trojan-跳过校验', type: trojan, server: t.example.com, port: 443, password: pw, skip-cert-verify: true }
"#;
        let outbounds = parse_clash_yaml(yaml).unwrap();
        assert_eq!(outbounds[0].raw_json["tls"]["insecure"], true);
        assert_eq!(outbounds[1].raw_json["tls"]["insecure"], true);
    }
}
