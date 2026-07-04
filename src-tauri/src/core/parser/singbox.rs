/// Sing-box 原生 JSON 订阅解析器
/// 作者: TanXiang
use super::ParsedOutbound;
use crate::error::AppError;
use serde_json::Value;

pub fn parse_singbox_json(content: &str) -> Result<Vec<ParsedOutbound>, AppError> {
    let parsed: Value = serde_json::from_str(content)
        .map_err(|e| AppError::Subscription(format!("Singbox JSON 语法错误: {}", e)))?;

    let mut outbounds = Vec::new();

    if let Some(arr) = parsed.as_array() {
        for item in arr {
            if let Some(outbound) = extract_outbound(item) {
                outbounds.push(outbound);
            }
        }
    } else if let Some(outbound_list) = parsed.get("outbounds").and_then(|v| v.as_array()) {
        for item in outbound_list {
            if let Some(outbound) = extract_outbound(item) {
                outbounds.push(outbound);
            }
        }
    }

    if outbounds.is_empty() {
        return Err(AppError::Subscription("Singbox 配置中没有找到有效节点".to_string()));
    }

    Ok(outbounds)
}

fn extract_outbound(item: &Value) -> Option<ParsedOutbound> {
    let tag = item.get("tag")?.as_str()?.to_string();
    let r#type = item.get("type")?.as_str()?.to_string();

    // 过滤选择器/直连/拦截等辅助出站类型
    if matches!(r#type.as_str(), "direct" | "block" | "dns" | "selector" | "urltest") {
        return None;
    }

    let server = item.get("server").and_then(|s| s.as_str()).map(|s| s.to_string());
    let server_port = item.get("server_port").and_then(|p| p.as_u64()).map(|p| p as u16);

    Some(ParsedOutbound {
        tag,
        r#type,
        server,
        server_port,
        raw_json: item.clone(),
    })
}
