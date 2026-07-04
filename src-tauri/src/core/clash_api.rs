/// Sing-box ClashAPI HTTP 客户端
/// 作者: TanXiang
use crate::error::AppError;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

pub struct ClashApiClient {
    client: Client,
    base_url: String,
}

impl ClashApiClient {
    pub fn new(base_url: Option<String>) -> Self {
        let base = base_url.unwrap_or_else(|| "http://127.0.0.1:9090".to_string());
        let client = Client::builder()
            .timeout(Duration::from_secs(6))
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
        let resp = self.client.get(&url).send().await
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

        let resp = self.client.get(&url).send().await
            .map_err(|e| AppError::Network(format!("延迟测试请求失败: {}", e)))?;

        let val: Value = resp.json().await
            .map_err(|e| AppError::Network(format!("解析延迟测试 JSON 失败: {}", e)))?;

        if let Some(delay) = val.get("delay").and_then(|d| d.as_u64()) {
            Ok(delay as u16)
        } else {
            Err(AppError::Network("测速超时或节点不可达".to_string()))
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
}

impl Default for ClashApiClient {
    fn default() -> Self {
        Self::new(None)
    }
}
