/// Sing-box ClashAPI HTTP 客户端
/// 作者: TanXiang
///
/// 对接 sing-box 运行时的 ClashAPI (127.0.0.1:9090)
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
            .timeout(Duration::from_secs(3))
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
