/// Sing-box ClashAPI HTTP 客户端
/// 作者: TanXiang
use crate::error::{ApiResponse, AppError};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::OnceLock;

pub static CLASH_API_PORT: AtomicU16 = AtomicU16::new(9090);

pub fn set_clash_api_port(port: u16) {
    CLASH_API_PORT.store(port, Ordering::Relaxed);
}

pub fn get_clash_api_port() -> u16 {
    CLASH_API_PORT.load(Ordering::Relaxed)
}

/// ClashAPI 访问令牌：首次生成随机值并原子落盘，之后所有请求统一携带 Bearer 头。
/// 修复原 secret 为空导致本机任意进程/浏览器跨站请求可完全控制内核的问题。
static CLASH_API_SECRET: OnceLock<String> = OnceLock::new();

pub fn get_clash_api_secret() -> &'static str {
    CLASH_API_SECRET.get_or_init(|| {
        let path = crate::get_data_root().join("clash_api_secret");
        if let Ok(s) = std::fs::read_to_string(&path) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        if let Err(e) = crate::fs_utils::atomic_write(&path, token.as_bytes()) {
            log::warn!("[clash_api] 持久化 ClashAPI secret 失败（本次会话使用内存值）: {}", e);
        }
        token
    })
}

/// 供前端 WebSocket 拼接 ?token= 参数使用（浏览器 WS 无法设置 Header）
#[tauri::command]
pub fn core_get_clash_secret() -> ApiResponse<String> {
    ApiResponse::ok(get_clash_api_secret().to_string())
}

pub struct ClashApiClient {
    client: Client,
    base_url: String,
}

/// /proxies 全量快照（1s TTL；select_node 写后清除保证读到最新 now）
static PROXIES_SNAPSHOT: std::sync::OnceLock<std::sync::Mutex<Option<(std::time::Instant, Value)>>> =
    std::sync::OnceLock::new();

impl ClashApiClient {
    pub fn new(base_url: Option<String>) -> Self {
        let port = get_clash_api_port();
        let base = base_url.unwrap_or_else(|| format!("http://127.0.0.1:{}", port));

        // 所有请求默认携带 Authorization: Bearer <secret>
        let mut headers = HeaderMap::new();
        let secret = get_clash_api_secret().to_string();
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", secret)) {
            headers.insert(AUTHORIZATION, v);
        }
        let client = Client::builder()
            .no_proxy()
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(50)
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url: base,
        }
    }

    /// 获取所有代理分组与节点
    ///
    /// 1 秒 TTL 快照复用：/proxies 全量 JSON 300 节点时约几百 KB，前端视图
    /// 挂载周期（fetchGroups + fetchGroupNodes）与单节点测速的 selector
    /// 查找会在同一秒内重复拉取——1s 内直接复用同一份快照。
    /// （切换节点 select_node 后的读取不受影响：写后清缓存）
    pub async fn get_proxies(&self) -> Result<Value, AppError> {
        let cache = PROXIES_SNAPSHOT.get_or_init(|| std::sync::Mutex::new(None));
        {
            let guard = cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((ts, cached)) = guard.as_ref() {
                if ts.elapsed() < Duration::from_secs(1) {
                    return Ok(cached.clone());
                }
            }
        }

        let url = format!("{}/proxies", self.base_url);
        let resp = self.client.get(&url)
            .send().await
            .map_err(|e| AppError::Network(format!("ClashAPI 请求失败: {}", e)))?;

        let status = resp.status();
        let body = resp.text().await
            .map_err(|e| AppError::Network(format!("ClashAPI 读取响应失败: {}", e)))?;
        if !status.is_success() {
            return Err(AppError::Network(format!("ClashAPI 请求失败: HTTP {} {}", status, body)));
        }
        let val: Value = serde_json::from_str(&body)
            .map_err(|e| AppError::Network(format!("ClashAPI 解析 JSON 失败: {}", e)))?;
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        *guard = Some((std::time::Instant::now(), val.clone()));
        Ok(val)
    }

    /// 使 /proxies 快照失效（select_node 等写操作后调用，保证下次读到最新 now）
    fn invalidate_proxies_snapshot(&self) {
        let cache = PROXIES_SNAPSHOT.get_or_init(|| std::sync::Mutex::new(None));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }

    /// 获取单节点延迟。
    /// sing-box 失败路径：504 超时 / 503 不可达 / 404 tag 不存在（body 携带 message），
    /// 先检查状态码再把服务端真实错误带回上层。
    pub async fn get_node_delay(&self, node_tag: &str, test_url: &str, timeout_ms: u64) -> Result<u16, AppError> {
        let encoded_tag = urlencoding::encode(node_tag);
        let encoded_url = urlencoding::encode(test_url);
        let url = format!(
            "{}/proxies/{}/delay?timeout={}&url={}",
            self.base_url, encoded_tag, timeout_ms, encoded_url
        );

        let resp = self.client.get(&url)
            .timeout(Duration::from_millis(timeout_ms + 5000))
            .send()
            .await
            .map_err(|e| AppError::Network(format!("延迟测试请求失败: {}", e)))?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.as_u16() == 404 {
            return Err(AppError::Network(format!("节点 [{}] 不存在（tag 编码错误或配置未加载）", node_tag)));
        }
        if !status.is_success() {
            let msg = serde_json::from_str::<Value>(&body).ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
                .unwrap_or(body.clone());
            return Err(AppError::Network(format!("延迟测试失败: HTTP {} {}", status, msg)));
        }
        let val: Value = serde_json::from_str(&body)
            .map_err(|e| AppError::Network(format!("解析延迟测试 JSON 失败: {}", e)))?;

        if let Some(delay) = val.get("delay").and_then(|d| d.as_u64()) {
            Ok(delay as u16)
        } else {
            Err(AppError::Network("测速超时或节点不可达".to_string()))
        }
    }

    /// 触发 URLTest 组的延迟测试。
    /// sing-box 的组测速端点返回 {map[tag]delay}（全部子节点的延迟映射，非单一 delay 值），
    /// 这里仅以 HTTP 状态判定触发是否成功（组已执行测速并自动重选），
    /// 子节点延迟由调用方另行通过 get_proxies / 缓存观察。
    pub async fn trigger_urltest_group_delay(&self, group_tag: &str, test_url: &str, timeout_ms: u64) -> Result<(), AppError> {
        let encoded_tag = urlencoding::encode(group_tag);
        let encoded_url = urlencoding::encode(test_url);
        let url = format!(
            "{}/proxies/{}/delay?timeout={}&url={}",
            self.base_url, encoded_tag, timeout_ms, encoded_url
        );

        let resp = self.client.get(&url)
            .timeout(Duration::from_millis(timeout_ms + 5000))
            .send()
            .await
            .map_err(|e| AppError::Network(format!("URLTest 组延迟测试请求失败: {}", e)))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            return Err(AppError::Network(format!("分组 [{}] 不存在（配置可能未按预期加载）", group_tag)));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let msg = serde_json::from_str::<Value>(&body).ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
                .unwrap_or(body.clone());
            return Err(AppError::Network(format!("URLTest 组测速失败: HTTP {} {}", status, msg)));
        }
        Ok(())
    }


    /// 切换 Selector 当前节点
    pub async fn select_node(&self, group_tag: &str, node_tag: &str) -> Result<(), AppError> {
        let url = format!("{}/proxies/{}", self.base_url, urlencoding::encode(group_tag));
        let body = serde_json::json!({ "name": node_tag });

        let resp = self.client.put(&url).json(&body).send().await
            .map_err(|e| AppError::Network(format!("切换节点请求失败: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Network(format!("切换节点返回错误状态: {}", resp.status())));
        }

        // 写后清快照：保证切换后立即读 /proxies 能拿到最新 now
        self.invalidate_proxies_snapshot();
        Ok(())
    }

    // 注意：不存在"配置热重载"能力——sing-box 的 PUT /configs 在全部版本（含 1.14）
    // 都是恒返回 204 的空实现，不会执行任何重载。让新 config.json 生效必须通过
    // 进程重启（本地：SidecarManager 重启；服务模式：RELOAD_CONFIG IPC 触发服务端
    // 停旧进程+起新进程）。原 reload_config 方法已删除，防止误用其"假成功"。
    // PATCH /configs 仅支持运行时字段（mode 等），用于模式热切换。

    /// 获取当前配置
    pub async fn get_configs(&self) -> Result<serde_json::Value, AppError> {
        let url = format!("{}/configs", self.base_url);
        let resp = self.client.get(&url).send().await
            .map_err(|e| AppError::Network(format!("获取配置失败: {}", e)))?;

        let val: serde_json::Value = resp.json().await
            .map_err(|e| AppError::Network(format!("解析配置失败: {}", e)))?;

        Ok(val)
    }

    /// 更新配置
    ///
    /// PATCH /configs 恒返回 204（即使 mode 非法也会静默 no-op）——
    /// 因此当 body 含 "mode" 时附带回读校验：GET /configs 比对 mode
    /// （大小写不敏感，sing-box 用 EqualFold 匹配），不一致视为热切换失败，
    /// 让调用方走自愈重启路径而不是拿到假成功。
    pub async fn patch_configs(&self, body: serde_json::Value) -> Result<(), AppError> {
        let url = format!("{}/configs", self.base_url);
        let resp = self.client.patch(&url).json(&body).send().await
            .map_err(|e| AppError::Network(format!("更新配置请求失败: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Network(format!("更新配置返回错误状态: {}", resp.status())));
        }

        // mode 热切换的假成功防护（204 ≠ 生效）
        if let Some(target_mode) = body.get("mode").and_then(|m| m.as_str()) {
            let applied = self.get_configs().await
                .ok()
                .and_then(|cfg| cfg.get("mode").and_then(|m| m.as_str()).map(|s| s.to_string()));
            match applied {
                Some(current) if current.eq_ignore_ascii_case(target_mode) => {}
                Some(current) => {
                    return Err(AppError::Network(format!(
                        "模式热切换未生效（目标 {}，内核仍为 {}）",
                        target_mode, current
                    )));
                }
                None => {
                    // 回读失败（内核刚重启 API 未就绪等）——不阻断，但记录告警
                    log::warn!("[clash_api] PATCH mode 后回读配置失败，跳过校验");
                }
            }
        }

        Ok(())
    }

    /// 关闭单条活跃连接

    pub async fn close_connection(&self, id: &str) -> Result<(), AppError> {
        let url = format!("{}/connections/{}", self.base_url, urlencoding::encode(id));
        let resp = self.client.delete(&url).send().await
            .map_err(|e| AppError::Network(format!("关闭连接请求失败: {}", e)))?;

        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::Network(format!("关闭连接返回错误状态: {}", resp.status())));
        }

        Ok(())
    }

    /// 关闭所有活跃连接
    pub async fn close_all_connections(&self) -> Result<(), AppError> {
        let url = format!("{}/connections", self.base_url);
        let resp = self.client.delete(&url).send().await
            .map_err(|e| AppError::Network(format!("关闭所有连接请求失败: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Network(format!("关闭所有连接返回错误状态: {}", resp.status())));
        }

        Ok(())
    }
}

impl Default for ClashApiClient {
    fn default() -> Self {
        // 进程级共享单例：此前每次 default() 都重建 reqwest Client
        //（builder 构建 + 连接池冷启动），代理页每次激活的多个 IPC 命令
        //（get_groups / get_mode / select_node…）各建一个客户端，
        // 连接池无法复用、TLS 握手重复，是页面进入卡顿的放大因素之一。
        // 端口动态（set_clash_api_port）在内核重启后变化：以当前端口为键
        // 缓存，端口变更时自动重建。
        let port = get_clash_api_port();
        static CACHED: OnceLock<std::sync::Mutex<Option<(u16, ClashApiClient)>>> =
            OnceLock::new();
        let cache = CACHED.get_or_init(|| std::sync::Mutex::new(None));
        let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cached_port, client)) = guard.as_ref() {
            if *cached_port == port {
                return Self {
                    client: client.client.clone(),
                    base_url: client.base_url.clone(),
                };
            }
        }
        let client = Self::new(None);
        *guard = Some((port, Self {
            client: client.client.clone(),
            base_url: client.base_url.clone(),
        }));
        client
    }
}
