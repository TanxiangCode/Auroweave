/// Clash / Mihomo YAML 订阅解析器
/// 作者: TanXiang
use super::ParsedOutbound;
use crate::error::AppError;
use serde_json::{json, Value};
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
            ("vmess".to_string(), json!({
                "type": "vmess",
                "tag": name,
                "server": server,
                "server_port": port,
                "uuid": uuid,
                "security": cipher,
                "alter_id": alter_id
            }))
        }
        "vless" => {
            let uuid = proxy.get("uuid")?.as_str()?.to_string();
            ("vless".to_string(), json!({
                "type": "vless",
                "tag": name,
                "server": server,
                "server_port": port,
                "uuid": uuid
            }))
        }
        "trojan" => {
            let password = proxy.get("password")?.as_str()?.to_string();
            ("trojan".to_string(), json!({
                "type": "trojan",
                "tag": name,
                "server": server,
                "server_port": port,
                "password": password
            }))
        }
        "hysteria2" | "hy2" => {
            let password = proxy.get("password").and_then(|v| v.as_str()).unwrap_or("");
            ("hysteria2".to_string(), json!({
                "type": "hysteria2",
                "tag": name,
                "server": server,
                "server_port": port,
                "password": password
            }))
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
