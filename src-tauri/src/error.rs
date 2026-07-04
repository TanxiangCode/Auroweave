/// Auroweave 统一错误类型
/// 作者: TanXiang
///
/// 规则：禁止在业务逻辑中使用 unwrap()/expect()，统一使用 Result<T, AppError>
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize, Deserialize, Clone)]
pub enum AppError {
    /// Sing-box 进程相关错误
    #[error("sing-box 进程错误: {0}")]
    Sidecar(String),

    /// 订阅拉取/解析错误
    #[error("订阅错误: {0}")]
    Subscription(String),

    /// 网络请求错误
    #[error("网络请求失败: {0}")]
    Network(String),

    /// 测速错误
    #[error("测速失败: {0}")]
    SpeedTest(String),

    /// 配置文件读写错误
    #[error("配置错误: {0}")]
    Config(String),

    /// 系统权限错误
    #[error("权限不足: {0}")]
    Permission(String),

    /// IO 错误
    #[error("IO 错误: {0}")]
    Io(String),

    /// 参数校验错误
    #[error("参数错误: {0}")]
    Validation(String),

    /// 未知错误
    #[error("未知错误: {0}")]
    Unknown(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Config(e.to_string())
    }
}

/// 统一 IPC 返回结构，与前端 ApiResponse<T> 对应
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub code: Option<i32>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            code: None,
        }
    }

    pub fn err(error: impl ToString, code: i32) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.to_string()),
            code: Some(code),
        }
    }
}

impl<T: Serialize> From<Result<T, AppError>> for ApiResponse<T> {
    fn from(result: Result<T, AppError>) -> Self {
        match result {
            Ok(data) => ApiResponse::ok(data),
            Err(e) => ApiResponse::err(&e, error_code(&e)),
        }
    }
}

/// 错误码映射（前端可据此做分类展示）
fn error_code(e: &AppError) -> i32 {
    match e {
        AppError::Permission(_) => 403,
        AppError::Network(_) => 502,
        AppError::Validation(_) => 400,
        AppError::Sidecar(_) => 500,
        _ => 500,
    }
}
