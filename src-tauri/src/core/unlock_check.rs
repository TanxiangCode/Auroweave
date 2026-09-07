/// AI 服务解锁检测引擎（Gemini / Claude / ChatGPT 地区封锁 + IP 风控近似判断）
/// 作者: TanXiang
///
/// 判据来源：lmc999/RegionRestrictionCheck（2026-09-06 实测复核）：
/// - Gemini：gemini.google.com 正文含 `45631641,null,true` = 可用（混淆 ID 会轮换，已 settings 化）
/// - Claude：claude.ai/ 重定向落 app-unavailable-in-region = 封锁；可用地区现在 302 到
///   claude.ai/login（原脚本"最终 URL 必须等于 claude.ai/"判据已过时，见实测修正）
/// - ChatGPT：api.openai.com/compliance/cookie_requirements 含 unsupported_country = 封锁
///
/// 检测必须走机制 B（selector 临时切换 + 经 mixed 端口的 reqwest）拿真实状态码/正文：
/// Gemini 封锁页是 HTTP 200、Claude 封锁是 302 落 200，机制 A（ClashAPI delay，
/// 非 2xx 才算失败）会把两者误报为可用。
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

/// 检测目标服务（新增服务 = 扩这个枚举 + 分类器 + 前端徽章映射）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnlockService {
    Gemini,
    Claude,
    Chatgpt,
}

impl UnlockService {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnlockService::Gemini => "gemini",
            UnlockService::Claude => "claude",
            UnlockService::Chatgpt => "chatgpt",
        }
    }

    /// 前端徽章字母（NodeCard 紧凑胶囊）
    pub fn badge(&self) -> &'static str {
        match self {
            UnlockService::Gemini => "G",
            UnlockService::Claude => "C",
            UnlockService::Chatgpt => "O",
        }
    }

    pub fn from_str_value(s: &str) -> Option<Self> {
        match s {
            "gemini" => Some(UnlockService::Gemini),
            "claude" => Some(UnlockService::Claude),
            "chatgpt" => Some(UnlockService::Chatgpt),
            _ => None,
        }
    }
}

/// 单服务检测结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnlockStatus {
    /// 服务在该出口可用
    Yes,
    /// 地区封锁（服务侧判定不支持当前地区）
    No,
    /// IP 风控疑似拦截（403 挑战 / 机房 IP 标记等启发式信号，非确定结论）
    Risky,
    /// 出口无法访问该服务（网络层失败/超时，与地区封锁区分）
    Failed,
}

/// 单节点一次检测的完整结果（随进度事件流式回传 + 落库）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockCheckResult {
    pub node_tag: String,
    /// 各服务检测结果
    pub services: std::collections::HashMap<String, UnlockStatus>,
    /// 出口 IP（ip-api 查询，可选层）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub egress_ip: Option<String>,
    /// 出口国家代码（ISO alpha-2）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    /// ip-api heuristic:出口为机房/托管 IP（Claude 风控的主要信号）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_flag: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isp: Option<String>,
    pub tested_at: i64,
}

/// 判据配置（settings 可更新——Gemini marker 是 Google 混淆 ID 会随版本轮换）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnlockCheckParams {
    /// Gemini 可用性正文 marker（默认 45631641,null,true）
    pub gemini_marker: String,
    /// Claude 地区封锁重定向特征（默认 app-unavailable-in-region）
    pub claude_block_marker: String,
    /// ChatGPT 封锁正文特征（默认 unsupported_country）
    pub chatgpt_block_marker: String,
}

impl Default for UnlockCheckParams {
    fn default() -> Self {
        Self {
            gemini_marker: "45631641,null,true".to_string(),
            claude_block_marker: "app-unavailable-in-region".to_string(),
            chatgpt_block_marker: "unsupported_country".to_string(),
        }
    }
}

impl UnlockCheckParams {
    pub fn from_settings(settings: &crate::commands::settings::AppSettings) -> Self {
        let d = Self::default();
        Self {
            gemini_marker: if settings.unlock_gemini_marker.trim().is_empty() {
                d.gemini_marker
            } else {
                settings.unlock_gemini_marker.clone()
            },
            claude_block_marker: if settings.unlock_claude_block_marker.trim().is_empty() {
                d.claude_block_marker
            } else {
                settings.unlock_claude_block_marker.clone()
            },
            chatgpt_block_marker: if settings.unlock_chatgpt_block_marker.trim().is_empty() {
                d.chatgpt_block_marker
            } else {
                settings.unlock_chatgpt_block_marker.clone()
            },
        }
    }
}

/// 经过 mixed 端口捕获的一次响应（分类器的输入；pub 仅为匹配 classify_response 的可见性）
pub struct ServiceResponse {
    status: u16,
    /// 跟随重定向后的最终 URL
    final_url: String,
    body: String,
}

/// 对单个服务的响应执行判据分类（纯函数，单测覆盖）
pub fn classify_response(
    service: UnlockService,
    resp: &ServiceResponse,
    params: &UnlockCheckParams,
) -> UnlockStatus {
    match service {
        UnlockService::Gemini => {
            // 封锁页同样返回 200（Google 通用"not available in your country"页），
            // 只能靠正文 marker 区分；连不上是网络层失败
            if resp.status == 200 {
                if resp.body.contains(&params.gemini_marker) {
                    UnlockStatus::Yes
                } else {
                    UnlockStatus::No
                }
            } else if resp.status == 403 {
                // 403 在 Gemini 语境是 IP 风控（Google 蜜罐 IP 拦截）而非地区封锁
                UnlockStatus::Risky
            } else {
                UnlockStatus::Failed
            }
        }
        UnlockService::Claude => {
            // 可用地区 302 → claude.ai/login（最终 URL 仍在 claude.ai 域且 2xx）
            if resp.final_url.contains("app-unavailable-in-region")
                || resp.final_url.contains(&params.claude_block_marker) {
                UnlockStatus::No
            } else if resp.status == 403 {
                // Cloudflare 数据center IP 拦截（近似判断，非确定）
                UnlockStatus::Risky
            } else if (200..300).contains(&resp.status)
                && resp.final_url.contains("claude.ai") {
                UnlockStatus::Yes
            } else {
                UnlockStatus::Failed
            }
        }
        UnlockService::Chatgpt => {
            if resp.status == 403 {
                // api.openai.com 403 = Cloudflare 风控拦截（经典 _cf_chl 挑战）
                UnlockStatus::Risky
            } else if (200..300).contains(&resp.status) {
                if resp.body.contains(&params.chatgpt_block_marker) {
                    UnlockStatus::No
                } else {
                    UnlockStatus::Yes
                }
            } else {
                UnlockStatus::Failed
            }
        }
    }
}

/// 经本地 mixed 端口请求服务页面（跟随重定向、读正文）。
/// 调用方负责先把 selector 切到目标节点——此函数只管"发出请求并取回响应"。
async fn fetch_service_response(
    client: &reqwest::Client,
    url: &str,
) -> Result<ServiceResponse, AppError> {
    let resp = client
        .get(url)
        .header("user-agent", BROWSER_UA)
        .header("accept-language", "en-US,en;q=0.9")
        .send()
        .await
        .map_err(|e| AppError::Network(format!("请求 {} 失败: {}", url, e)))?;

    let status = resp.status().as_u16();
    let final_url = resp.url().to_string();
    // 正文读上限 512KB：Gemini 首页 ~360KB 够用，防异常大响应拖内存
    let body = resp
        .bytes()
        .await
        .map_err(|e| AppError::Network(format!("读取 {} 响应失败: {}", url, e)))?;
    let body = String::from_utf8_lossy(&body).to_string();

    Ok(ServiceResponse {
        status,
        final_url,
        body: body.chars().take(512 * 1024).collect(),
    })
}

/// 出口信息聚合（IP/归属国/hosting/proxy/ISP）
#[derive(Debug, Clone, Default)]
pub struct EgressInfo {
    pub ip: Option<String>,
    pub country_code: Option<String>,
    pub hosting: Option<bool>,
    pub proxy_flag: Option<bool>,
    pub isp: Option<String>,
}

/// ip-api 出口信息查询（经出口端口，与连通性检测同一数据源形态）。
///
/// 免费限 45 req/min **按出口 IP 计数**——批量检测下每查询来自各节点自己的
/// 出口，天然分散；唯一例外是机场同出口挂多节点（单 IP 累积）。触限（429）
/// 时换 freeipapi 单发重试一次（60 req/min、含 isProxy；hosting 字段缺失置
/// None——proxy 布尔可顶替机房启发式语义）。
pub async fn fetch_egress_info(client: &reqwest::Client) -> Result<EgressInfo, AppError> {
    let resp = client
        .get("http://ip-api.com/json?fields=query,countryCode,isp,proxy,hosting&lang=zh-CN")
        .timeout(Duration::from_secs(8))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().as_u16() == 429 => {
            // 限速容错：freeipapi 单发重试（调研实测 2026-09-07：无 key、含 isProxy）
            fetch_egress_info_freeipapi(client).await
        }
        Ok(r) if r.status().is_success() => {
            let v: serde_json::Value = r
                .json()
                .await
                .map_err(|e| AppError::Network(format!("解析 ip-api 响应失败: {}", e)))?;
            Ok(EgressInfo {
                ip: str_field(&v, "query"),
                country_code: str_field(&v, "countryCode"),
                hosting: bool_field(&v, "hosting"),
                proxy_flag: bool_field(&v, "proxy"),
                isp: str_field(&v, "isp"),
            })
        }
        Ok(r) => Err(AppError::Network(format!("ip-api 返回 HTTP {}", r.status()))),
        Err(e) => Err(AppError::Network(format!("ip-api 查询失败: {}", e))),
    }
}

/// freeipapi 备源（ip-api 429 时的单发重试；无 hosting 字段，isProxy 顶替）
async fn fetch_egress_info_freeipapi(client: &reqwest::Client) -> Result<EgressInfo, AppError> {
    let resp = client
        .get("https://freeipapi.com/api/json")
        .timeout(Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| AppError::Network(format!("freeipapi 查询失败: {}", e)))?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!("freeipapi 返回 HTTP {}", resp.status())));
    }
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Network(format!("解析 freeipapi 响应失败: {}", e)))?;
    Ok(EgressInfo {
        ip: str_field(&v, "ipAddress"),
        country_code: str_field(&v, "countryCode"),
        hosting: None, // freeipapi 无机房字段；isProxy 覆盖代理/VPN/机房启发式
        proxy_flag: bool_field(&v, "isProxy"),
        isp: str_field(&v, "asnOrganization"),
    })
}

fn str_field(v: &serde_json::Value, k: &str) -> Option<String> {
    v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn bool_field(v: &serde_json::Value, k: &str) -> Option<bool> {
    v.get(k).and_then(|x| x.as_bool())
}

/// 服务探测 URL
pub fn service_url(service: UnlockService) -> &'static str {
    match service {
        UnlockService::Gemini => "https://gemini.google.com",
        UnlockService::Claude => "https://claude.ai/",
        UnlockService::Chatgpt => "https://api.openai.com/compliance/cookie_requirements",
    }
}

/// 对"当前出口"执行一轮完整检测。
///
/// `egress_port`：流量出口的本地 mixed 端口——主实例（selector 已被调用方
/// 切到目标节点）传 mixed_port；test-core 专属端口（inbound 规则已钉死到
/// 目标节点）传 port_base+i。检测语义两种通道完全一致。
/// services 为空时只做 ip 层；with_ip=false 跳过 ip 层。
///
/// 三服务 + ip 层全部并行：单服务经代理远端往返 2-5s，串行三服务 10-15s
/// 是单节点检测的耗时大头；各探测相互独立无共享状态，墙钟时间收敛到
/// 最慢一路（约 5-8s）。
pub async fn check_current_exit(
    egress_port: u16,
    services: &[UnlockService],
    with_ip: bool,
    params: &UnlockCheckParams,
) -> Result<UnlockCheckResult, AppError> {
    let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{}", egress_port))
        .map_err(|e| AppError::Network(format!("创建本地代理客户端失败: {}", e)))?;
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Network(e.to_string()))?;

    // 并行任务组：探测枚举 + ip 层（JoinSet 要求同型任务，用 ProbeOutcome 分派）
    enum ProbeOutcome {
        Service(String, UnlockStatus),
        Ip(Result<EgressInfo, AppError>),
    }
    let mut join_set = tokio::task::JoinSet::new();
    for &service in services {
        join_set.spawn({
            let client = client.clone();
            let params = params.clone();
            async move {
                let status = match fetch_service_response(&client, service_url(service)).await {
                    Ok(resp) => classify_response(service, &resp, &params),
                    Err(_) => UnlockStatus::Failed,
                };
                ProbeOutcome::Service(service.as_str().to_string(), status)
            }
        });
    }
    if with_ip {
        join_set.spawn({
            let client = client.clone();
            async move { ProbeOutcome::Ip(fetch_egress_info(&client).await) }
        });
    }

    let mut service_results = Vec::with_capacity(services.len());
    let mut ip_result: Option<EgressInfo> = None;
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(ProbeOutcome::Service(name, status)) => service_results.push((name, status)),
            Ok(ProbeOutcome::Ip(Ok(info))) => ip_result = Some(info),
            Ok(ProbeOutcome::Ip(Err(e))) => {
                // ip 层失败不判 Failed——服务层结果依然有效，仅 IP 层缺省
                log::debug!("[unlock] ip 层查询失败（结果保留服务层字段）: {}", e);
            }
            Err(e) => log::debug!("[unlock] 并行探测任务异常: {}", e),
        }
    }

    let mut result = UnlockCheckResult {
        node_tag: String::new(), // 由调用方回填
        services: std::collections::HashMap::new(),
        egress_ip: None,
        country_code: None,
        hosting: None,
        proxy_flag: None,
        isp: None,
        tested_at: chrono::Utc::now().timestamp_millis(),
    };
    for (name, status) in service_results {
        result.services.insert(name, status);
    }
    // ip 层失败不判 Failed——服务层结果依然有效，仅 IP 层缺省
    if let Some(info) = ip_result {
        result.egress_ip = info.ip;
        result.country_code = info.country_code;
        result.hosting = info.hosting;
        result.proxy_flag = info.proxy_flag;
        result.isp = info.isp;
    }

    Ok(result)
}

/// 为单元测试暴露的响应构造器（生产代码不使用）
#[cfg(test)]
pub(crate) fn test_response(status: u16, final_url: &str, body: &str) -> ServiceResponse {
    ServiceResponse {
        status,
        final_url: final_url.to_string(),
        body: body.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> UnlockCheckParams {
        UnlockCheckParams::default()
    }

    #[test]
    fn gemini_available_marker() {
        let body = r#"},[45772048,null,false],[45631641,null,true,"XgpeRd"],"#;
        let resp = test_response(200, "https://gemini.google.com/", body);
        assert_eq!(
            classify_response(UnlockService::Gemini, &resp, &params()),
            UnlockStatus::Yes
        );
    }

    #[test]
    fn gemini_region_blocked_page_is_200_without_marker() {
        // Gemini 封锁页同样返回 200，正文无 marker
        let body = "<html>Not available in your country</html>";
        let resp = test_response(200, "https://gemini.google.com/", body);
        assert_eq!(
            classify_response(UnlockService::Gemini, &resp, &params()),
            UnlockStatus::No
        );
    }

    #[test]
    fn gemini_403_is_ip_risk_control() {
        let resp = test_response(403, "https://gemini.google.com/", "");
        assert_eq!(
            classify_response(UnlockService::Gemini, &resp, &params()),
            UnlockStatus::Risky
        );
    }

    #[test]
    fn gemini_network_error_is_failed() {
        let resp = test_response(502, "https://gemini.google.com/", "");
        assert_eq!(
            classify_response(UnlockService::Gemini, &resp, &params()),
            UnlockStatus::Failed
        );
    }

    #[test]
    fn claude_available_redirects_to_login() {
        // 实测修正：可用地区现在 302 → claude.ai/login（原脚本判据已过时）
        let resp = test_response(200, "https://claude.ai/login", "");
        assert_eq!(
            classify_response(UnlockService::Claude, &resp, &params()),
            UnlockStatus::Yes
        );
    }

    #[test]
    fn claude_region_block_redirect() {
        let resp = test_response(
            200,
            "https://www.anthropic.com/app-unavailable-in-region",
            "",
        );
        assert_eq!(
            classify_response(UnlockService::Claude, &resp, &params()),
            UnlockStatus::No
        );
    }

    #[test]
    fn claude_cloudflare_403_is_risky() {
        let resp = test_response(403, "https://claude.ai/", "Just a moment...");
        assert_eq!(
            classify_response(UnlockService::Claude, &resp, &params()),
            UnlockStatus::Risky
        );
    }

    #[test]
    fn claude_unreachable_is_failed() {
        let resp = test_response(502, "https://claude.ai/", "");
        assert_eq!(
            classify_response(UnlockService::Claude, &resp, &params()),
            UnlockStatus::Failed
        );
    }

    #[test]
    fn chatgpt_available() {
        let resp = test_response(200, "https://api.openai.com/compliance/cookie_requirements", "{}");
        assert_eq!(
            classify_response(UnlockService::Chatgpt, &resp, &params()),
            UnlockStatus::Yes
        );
    }

    #[test]
    fn chatgpt_unsupported_country() {
        let body = r#"{"error":"unsupported_country"}"#;
        let resp = test_response(200, "https://api.openai.com/compliance/cookie_requirements", body);
        assert_eq!(
            classify_response(UnlockService::Chatgpt, &resp, &params()),
            UnlockStatus::No
        );
    }

    #[test]
    fn chatgpt_cf_challenge_403() {
        let resp = test_response(403, "https://api.openai.com/", "_cf_chl_opt");
        assert_eq!(
            classify_response(UnlockService::Chatgpt, &resp, &params()),
            UnlockStatus::Risky
        );
    }

    #[test]
    fn custom_marker_from_settings_overrides_default() {
        // Google 轮换混淆 ID 后用户改 settings——新 marker 必须立即生效
        let mut p = params();
        p.gemini_marker = "99999999,null,true".to_string();
        let resp = test_response(200, "https://gemini.google.com/", "xx99999999,null,trueyy");
        assert_eq!(
            classify_response(UnlockService::Gemini, &resp, &p),
            UnlockStatus::Yes
        );
        // 默认 marker 不再命中
        assert_eq!(
            classify_response(UnlockService::Gemini, &resp, &params()),
            UnlockStatus::No
        );
    }

    #[test]
    fn service_roundtrip() {
        for s in [UnlockService::Gemini, UnlockService::Claude, UnlockService::Chatgpt] {
            assert_eq!(UnlockService::from_str_value(s.as_str()), Some(s));
        }
        assert_eq!(UnlockService::from_str_value("nope"), None);
    }
}
