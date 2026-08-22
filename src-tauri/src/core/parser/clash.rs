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

    for proxy in proxies_arr {
        if let Some(parsed) = convert_clash_proxy_to_singbox(proxy) {
            outbounds.push(parsed);
        }
    }

    if outbounds.is_empty() {
        return Err(AppError::Subscription("Clash 订阅中未找到有效节点".to_string()));
    }

    Ok(outbounds)
}

fn convert_clash_proxy_to_singbox(proxy: &YamlValue) -> Option<ParsedOutbound> {
    let name = proxy.get("name")?.as_str()?.to_string();
    let proxy_type = proxy.get("type")?.as_str()?.to_lowercase();
    let server = proxy.get("server")?.as_str()?.to_string();
    let port = proxy.get("port")?.as_u64()? as u16;

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
            let alter_id = proxy.get("alterId").and_then(|v| v.as_u64()).unwrap_or(0);
            let cipher = proxy.get("cipher").and_then(|v| v.as_str()).unwrap_or("auto");
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
                    let mut tls_obj = json!({ "enabled": true });
                    if let Some(s) = sni { tls_obj["server_name"] = json!(s); }
                    obj["tls"] = tls_obj;
                }
            }
            if let Some(net) = proxy.get("network").and_then(|v| v.as_str()) {
                if net == "ws" {
                    let path = proxy.get("ws-opts").and_then(|v| v.get("path")).and_then(|v| v.as_str()).unwrap_or("/");
                    obj["transport"] = json!({ "type": "ws", "path": path });
                } else if net == "grpc" {
                    let service_name = proxy.get("grpc-opts").and_then(|v| v.get("grpc-service-name")).and_then(|v| v.as_str()).unwrap_or("");
                    obj["transport"] = json!({ "type": "grpc", "service_name": service_name });
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
                if !flow.is_empty() { obj["flow"] = json!(flow); }
            }
            if let Some(reality_opts) = proxy.get("reality-opts") {
                let public_key = reality_opts.get("public-key").and_then(|v| v.as_str()).unwrap_or("");
                let short_id = reality_opts.get("short-id").and_then(|v| v.as_str()).unwrap_or("");
                let sni = proxy.get("servername").or_else(|| proxy.get("sni")).and_then(|v| v.as_str());
                let mut reality = json!({
                    "enabled": true,
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
                let mut tls_obj = json!({ "enabled": true });
                if let Some(s) = sni { tls_obj["server_name"] = json!(s); }
                obj["tls"] = tls_obj;
            }
            ("vless".to_string(), obj)
        }
        "trojan" => {
            let password = proxy.get("password")?.as_str()?.to_string();
            let sni = proxy.get("sni").or_else(|| proxy.get("servername")).and_then(|v| v.as_str()).unwrap_or(&server);
            let mut obj = json!({
                "type": "trojan",
                "tag": name,
                "server": server,
                "server_port": port,
                "password": password,
                "tls": {
                    "enabled": true,
                    "server_name": sni
                }
            });
            if let Some(net) = proxy.get("network").and_then(|v| v.as_str()) {
                if net == "ws" {
                    let path = proxy.get("ws-opts").and_then(|v| v.get("path")).and_then(|v| v.as_str()).unwrap_or("/");
                    obj["transport"] = json!({ "type": "ws", "path": path });
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
            let sni = proxy.get("sni").or_else(|| proxy.get("servername")).and_then(|v| v.as_str());
            if let Some(s) = sni {
                obj["tls"] = json!({ "enabled": true, "server_name": s });
            }
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
            }
            if let Some(alpn_seq) = proxy.get("alpn").and_then(|v| v.as_sequence()) {
                let alpn_list: Vec<String> = alpn_seq.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                if !alpn_list.is_empty() {
                    tls_obj["alpn"] = json!(alpn_list);
                }
            }

            ("vless".to_string(), json!({
                "type": "vless",
                "tag": name,
                "server": server,
                "server_port": port,
                "uuid": password,
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
        assert_eq!(node.r#type, "vless");
        assert_eq!(node.server, Some("zf-tw1.9999231.xyz".to_string()));
        assert_eq!(node.server_port, Some(500));
        assert_eq!(node.raw_json["tls"]["enabled"], true);
        assert_eq!(node.raw_json["tls"]["server_name"], "tw01.9999231.xyz");
        assert_eq!(node.raw_json["tls"]["utls"]["fingerprint"], "chrome");
    }
}
