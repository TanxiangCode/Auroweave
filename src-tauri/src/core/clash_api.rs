/// Sing-box ClashAPI HTTP 客户端
/// 作者: TanXiang
use crate::error::AppError;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use std::sync::atomic::{AtomicU16, Ordering};

pub static CLASH_API_PORT: AtomicU16 = AtomicU16::new(9090);

pub fn set_clash_api_port(port: u16) {
    CLASH_API_PORT.store(port, Ordering::Relaxed);
}

pub fn get_clash_api_port() -> u16 {
    CLASH_API_PORT.load(Ordering::Relaxed)
}

pub struct ClashApiClient {
    client: Client,
    base_url: String,
}

impl ClashApiClient {
    pub fn new(base_url: Option<String>) -> Self {
        let port = get_clash_api_port();
        let base = base_url.unwrap_or_else(|| format!("http://127.0.0.1:{}", port));
        let client = Client::builder()
            .no_proxy()
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
    pub async fn get_proxies(&self) -> Result<Value, AppError> {
        let url = format!("{}/proxies", self.base_url);
        let resp = self.client.get(&url)
            .timeout(Duration::from_secs(10))
            .send().await
            .map_err(|e| AppError::Network(format!("ClashAPI 请求失败: {}", e)))?;

        let val: Value = resp.json().await
            .map_err(|e| AppError::Network(format!("ClashAPI 解析 JSON 失败: {}", e)))?;

        Ok(val)
    }

    /// 获取单节点延迟
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

        let val: Value = resp.json().await
            .map_err(|e| AppError::Network(format!("解析延迟测试 JSON 失败: {}", e)))?;

        if let Some(delay) = val.get("delay").and_then(|d| d.as_u64()) {
            Ok(delay as u16)
        } else {
            Err(AppError::Network("测速超时或节点不可达".to_string()))
        }
    }

    /// 触发 URLTest 组的延迟测试（自动选择最优节点）
    /// 返回选择的节点延迟（用于验证测试是否成功）
    pub async fn trigger_urltest_group_delay(&self, group_tag: &str, test_url: &str, timeout_ms: u64) -> Result<u16, AppError> {
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

        let val: Value = resp.json().await
            .map_err(|e| AppError::Network(format!("解析 URLTest 组延迟测试 JSON 失败: {}", e)))?;

        if let Some(delay) = val.get("delay").and_then(|d| d.as_u64()) {
            Ok(delay as u16)
        } else {
            Err(AppError::Network("URLTest 组测速超时或不可达".to_string()))
        }
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

        Ok(())
    }

    /// 热重载配置
    pub async fn reload_config(&self, config_path: &str) -> Result<(), AppError> {
        let url = format!("{}/configs?force=true", self.base_url);
        let body = serde_json::json!({
            "path": config_path
        });

        let resp = self.client.put(&url).json(&body).send().await
            .map_err(|e| AppError::Network(format!("热重载请求失败: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Network(format!("热重载返回错误状态: {}", resp.status())));
        }

        Ok(())
    }

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
    pub async fn patch_configs(&self, body: serde_json::Value) -> Result<(), AppError> {
        let url = format!("{}/configs", self.base_url);
        let resp = self.client.patch(&url).json(&body).send().await
            .map_err(|e| AppError::Network(format!("更新配置请求失败: {}", e)))?;

        if !resp.status().is_success() {
            return Err(AppError::Network(format!("更新配置返回错误状态: {}", resp.status())));
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
        Self::new(None)
    }
}
