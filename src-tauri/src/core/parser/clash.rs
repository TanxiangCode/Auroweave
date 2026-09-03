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

/// 构建 Clash YAML 的 multiplex JSON（vmess/vless/trojan 等共用）。
/// smux 编码（Mihomo 生态）→ sing-box protocol；max-connections/min-streams/max-streams 透传。
/// 对应 docs/sing-box_docs/shared/multiplex.md：max_connections/min_streams 与 max_streams 互斥。
fn build_clash_multiplex(proxy: &YamlValue) -> Option<serde_json::Value> {
    let smux = proxy.get("smux")?;
    if smux.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false) {
        let protocol = match smux.get("protocol").and_then(|v| v.as_str()).unwrap_or("smux") {
            "yamux" => "yamux",
            "h2mux" => "h2mux",
            _ => "smux",
        };
        let mut m = json!({ "enabled": true, "protocol": protocol });
        if let Some(n) = smux.get("max-connections").and_then(|v| v.as_u64()) {
            m["max_connections"] = json!(n);
        }
        if let Some(n) = smux.get("min-streams").and_then(|v| v.as_u64()) {
            m["min_streams"] = json!(n);
        }
        if let Some(n) = smux.get("max-streams").and_then(|v| v.as_u64()) {
            m["max_streams"] = json!(n);
        }
        if smux.get("padding").and_then(|v| v.as_bool()).unwrap_or(false) {
            m["padding"] = json!(true);
        }
        Some(m)
    } else {
        None
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
            if let Some(multiplex) = build_clash_multiplex(proxy) {
                obj["multiplex"] = multiplex;
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
            if let Some(multiplex) = build_clash_multiplex(proxy) {
                obj["multiplex"] = multiplex;
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
            if let Some(multiplex) = build_clash_multiplex(proxy) {
                obj["multiplex"] = multiplex;
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
            // 端口跳跃（outbound/hysteria2.md：server_ports 列表与 server_port 互斥）：
            // Mihomo 的 ports 形如 "2080:3000" 字符串或 ["2080:3000","3001"] 列表；
            // 设置后移除 server_port
            let ports_raw = proxy.get("ports");
            let ports_seq: Option<Vec<String>> = match ports_raw {
                Some(YamlValue::Sequence(seq)) => Some(
                    seq.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect(),
                ),
                Some(YamlValue::String(s)) if !s.trim().is_empty() => {
                    Some(vec![s.trim().to_string()])
                }
                _ => None,
            };
            if let Some(ports) = ports_seq {
                if !ports.is_empty() {
                    obj["server_ports"] = json!(ports);
                    obj.as_object_mut().unwrap().remove("server_port");
                }
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
        "tuic" => {
            let uuid = proxy.get("uuid")?.as_str()?.to_string();
            let password = proxy.get("password").and_then(|v| v.as_str()).unwrap_or("");
            // 拥塞控制白名单（outbound/tuic.md：cubic/new_reno/bbr），非法值回退 cubic
            const TUIC_CC: &[&str] = &["cubic", "new_reno", "bbr"];
            let cc = proxy
                .get("congestion-controller")
                .or_else(|| proxy.get("congestion_control"))
                .and_then(|v| v.as_str())
                .unwrap_or("cubic");
            let cc = if TUIC_CC.contains(&cc) { cc } else {
                log::warn!("[parser] Clash tuic 节点 [{}] 的非法拥塞控制 {} 回退为 cubic", name, cc);
                "cubic"
            };
            // udp_relay_mode 白名单（native/quic）
            let udp_mode = proxy
                .get("udp-relay-mode")
                .or_else(|| proxy.get("udp_relay_mode"))
                .and_then(|v| v.as_str())
                .unwrap_or("native");
            let udp_mode = if udp_mode == "native" || udp_mode == "quic" { udp_mode } else { "native" };

            // TUIC 始终基于 QUIC+TLS：缺省 sni 回退 server，缺省 alpn h3
            let sni = proxy.get("sni").or_else(|| proxy.get("servername")).and_then(|v| v.as_str());
            let insecure = proxy.get("skip-cert-verify").and_then(|v| v.as_bool()).unwrap_or(false);
            let alpn = proxy
                .get("alpn")
                .and_then(|v| v.as_sequence())
                .map(|seq| seq.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<String>>())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| vec!["h3".to_string()]);

            let mut tls_obj = json!({
                "enabled": true,
                "alpn": alpn,
                "insecure": insecure
            });
            tls_obj["server_name"] = json!(sni.unwrap_or(&server));

            let mut obj = json!({
                "type": "tuic",
                "tag": name,
                "server": server,
                "server_port": port,
                "uuid": uuid,
                "congestion_control": cc,
                "udp_relay_mode": udp_mode,
                "tls": tls_obj
            });
            if !password.is_empty() { obj["password"] = json!(password); }
            if let Some(ip_prefer) = proxy.get("ip-version") {
                // Mihomo ip-version 映射到 sing-box network 并不语义等价，忽略即可
                let _ = ip_prefer;
            }
            ("tuic".to_string(), obj)
        }
        "snell" => {
            // sing-box 1.14.0 新增原生 snell 支持（此前全部拒载）
            let psk = proxy.get("psk").and_then(|v| v.as_str()).unwrap_or("");
            if psk.is_empty() {
                log::warn!("[parser] Clash snell 节点 [{}] 缺少 psk，跳过", name);
                return None;
            }
            // 版本必填且白名单 4/6（obfs 仅 v4 支持；v5 QUIC 模式官方明确不支持）
            let version = proxy.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
            if version != 4 && version != 6 {
                log::warn!("[parser] Clash snell 节点 [{}] 版本 {} 不受支持（仅 4/6），跳过", name, version);
                return None;
            }
            let mut obj = json!({
                "type": "snell",
                "tag": name,
                "server": server,
                "server_port": port,
                "version": version,
                "psk": psk
            });
            if version == 4 {
                // v4 支持 HTTP 混淆：obfs-mode http/none，obfs-host 缺省 bing.com
                let obfs_mode = proxy.get("obfs-mode").and_then(|v| v.as_str()).unwrap_or("none");
                if obfs_mode == "http" {
                    let obfs_host = proxy.get("obfs-host").and_then(|v| v.as_str()).unwrap_or("bing.com");
                    obj["obfs_mode"] = json!("http");
                    obj["obfs_host"] = json!(obfs_host);
                }
            }
            ("snell".to_string(), obj)
        }
        "hysteria" => {
            // hysteria v1（QUIC）：up/down 带宽必填（服务端 Brutal 限速依赖），auth_str 认证
            let auth_str = proxy.get("auth_str").or_else(|| proxy.get("auth-str")).and_then(|v| v.as_str()).unwrap_or("");
            let up = proxy.get("up").and_then(|v| v.as_str());
            let down = proxy.get("down").and_then(|v| v.as_str());
            let sni = proxy.get("sni").and_then(|v| v.as_str()).unwrap_or(&server);
            let insecure = proxy.get("skip-cert-verify").and_then(|v| v.as_bool()).unwrap_or(false);
            let mut tls_obj = json!({ "enabled": true, "server_name": sni });
            if insecure { tls_obj["insecure"] = json!(true); }
            let alpn = proxy.get("alpn").and_then(|v| v.as_sequence())
                .map(|seq| seq.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<String>>());
            if let Some(a) = alpn { tls_obj["alpn"] = json!(a); }

            let mut obj = json!({
                "type": "hysteria",
                "tag": name,
                "server": server,
                "server_port": port,
                "tls": tls_obj
            });
            if !auth_str.is_empty() { obj["auth_str"] = json!(auth_str); }
            if let Some(u) = up { obj["up"] = json!(u); }
            if let Some(d) = down { obj["down"] = json!(d); }
            if let Some(obfs) = proxy.get("obfs").and_then(|v| v.as_str()) {
                if !obfs.is_empty() { obj["obfs"] = json!(obfs); }
            }
            ("hysteria".to_string(), obj)
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

    #[test]
    fn test_parse_clash_tuic() {
        let yaml = r#"
proxies:
  - { name: 'tuic-1', type: tuic, server: t.example.com, port: 443, uuid: 2DD61D93-75D8-4DA4-AC0E-6AECE7EAC365, password: hello, sni: t.example.com, congestion-controller: bbr, udp-relay-mode: native, alpn: [h3] }
  - { name: 'tuic-非法cc', type: tuic, server: t2.example.com, port: 443, uuid: uuid-x, congestion-controller: bogus }
"#;
        let outbounds = parse_clash_yaml(yaml).unwrap();
        assert_eq!(outbounds.len(), 2);
        let n = &outbounds[0];
        assert_eq!(n.r#type, "tuic");
        assert_eq!(n.raw_json["uuid"], "2DD61D93-75D8-4DA4-AC0E-6AECE7EAC365");
        assert_eq!(n.raw_json["password"], "hello");
        assert_eq!(n.raw_json["congestion_control"], "bbr");
        assert_eq!(n.raw_json["udp_relay_mode"], "native");
        assert_eq!(n.raw_json["tls"]["alpn"][0], "h3");
        // 非法拥塞控制回退 cubic（不能透传坏值拒载整份配置）
        assert_eq!(outbounds[1].raw_json["congestion_control"], "cubic");
    }

    #[test]
    fn test_parse_clash_snell() {
        let yaml = r#"
proxies:
  - { name: 'snell-4', type: snell, server: s.example.com, port: 443, psk: psk123, version: 4, obfs-mode: http, obfs-host: bing.com }
  - { name: 'snell-6', type: snell, server: s6.example.com, port: 443, psk: psk456, version: 6 }
  - { name: 'snell-无psk', type: snell, server: s.example.com, port: 443, version: 4 }
  - { name: 'snell-v5', type: snell, server: s.example.com, port: 443, psk: psk, version: 5 }
"#;
        let outbounds = parse_clash_yaml(yaml).unwrap();
        // 缺 psk 与 v5 都被跳过，仅保留 v4/v6
        assert_eq!(outbounds.len(), 2);
        assert_eq!(outbounds[0].r#type, "snell");
        assert_eq!(outbounds[0].raw_json["version"], 4);
        assert_eq!(outbounds[0].raw_json["psk"], "psk123");
        assert_eq!(outbounds[0].raw_json["obfs_mode"], "http");
        assert_eq!(outbounds[0].raw_json["obfs_host"], "bing.com");
        assert_eq!(outbounds[1].raw_json["version"], 6);
    }

    #[test]
    fn test_parse_clash_hysteria_v1() {
        let yaml = r#"
proxies:
  - { name: 'hy1-1', type: hysteria, server: h.example.com, port: 443, auth_str: authtoken, up: '100 Mbps', down: '200 Mbps', sni: h.example.com, skip-cert-verify: true, obfs: obfskey }
"#;
        let outbounds = parse_clash_yaml(yaml).unwrap();
        let n = &outbounds[0];
        assert_eq!(n.r#type, "hysteria");
        assert_eq!(n.raw_json["auth_str"], "authtoken");
        assert_eq!(n.raw_json["up"], "100 Mbps");
        assert_eq!(n.raw_json["down"], "200 Mbps");
        assert_eq!(n.raw_json["obfs"], "obfskey");
        assert_eq!(n.raw_json["tls"]["insecure"], true);
        assert_eq!(n.raw_json["tls"]["server_name"], "h.example.com");
    }

    #[test]
    fn test_parse_clash_hy2_ports_hopping() {
        // ports 字段（Mihomo 端口跳跃）→ server_ports，并移除互斥的 server_port
        let yaml = r#"
proxies:
  - { name: 'hy2-hop', type: hysteria2, server: h2.example.com, port: 443, password: pw, ports: '2080:3000', sni: h2.example.com }
"#;
        let outbounds = parse_clash_yaml(yaml).unwrap();
        let raw = &outbounds[0].raw_json;
        assert_eq!(raw["server_ports"][0], "2080:3000");
        assert!(raw.get("server_port").is_none(), "server_port 与 server_ports 互斥，必须移除");
    }

    #[test]
    fn test_parse_clash_smux_multiplex() {
        let yaml = r#"
proxies:
  - { name: 'vmess-smux', type: vmess, server: v.example.com, port: 443, uuid: uuid-x, alterId: 0, cipher: auto, smux: { enabled: true, protocol: smux, max-connections: 4, min-streams: 4 } }
  - { name: 'trojan-yamux', type: trojan, server: t.example.com, port: 443, password: pw, smux: { enabled: true, protocol: yamux, padding: true } }
  - { name: 'vmess-无smux', type: vmess, server: v2.example.com, port: 443, uuid: uuid-y, alterId: 0, cipher: auto }
"#;
        let outbounds = parse_clash_yaml(yaml).unwrap();
        let m1 = &outbounds[0].raw_json["multiplex"];
        assert_eq!(m1["enabled"], true);
        assert_eq!(m1["protocol"], "smux");
        assert_eq!(m1["max_connections"], 4);
        assert_eq!(m1["min_streams"], 4);
        assert_eq!(outbounds[1].raw_json["multiplex"]["protocol"], "yamux");
        assert_eq!(outbounds[1].raw_json["multiplex"]["padding"], true);
        // 未启用 smux 的节点不应生成 multiplex 块
        assert!(outbounds[2].raw_json.get("multiplex").is_none());
    }
}
