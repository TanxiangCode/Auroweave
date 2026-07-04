/// sing-box sidecar 生命周期管理
/// 作者: TanXiang
///
/// 职责：
/// - 以独立子进程拉起 sing-box 二进制（版本：1.13.14）
/// - 监听进程退出，自动重启
/// - 响应系统挂起（Sleep）与唤醒（Wake）信号，自动重连
/// - 提供停止接口（应用退出时调用）
use crate::error::AppError;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::process::Child;
use tracing::{info, warn, error};

/// sing-box 锁定版本
pub const SINGBOX_VERSION: &str = "1.13.14";

/// Sidecar 状态
#[derive(Debug, Clone, PartialEq)]
pub enum SidecarStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

/// Sidecar 管理器（线程安全）
pub struct SidecarManager {
    process: Arc<Mutex<Option<Child>>>,
    status: Arc<Mutex<SidecarStatus>>,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(SidecarStatus::Stopped)),
        }
    }

    /// 启动 sing-box 进程
    ///
    /// config_path: sing-box config.json 的绝对路径
    pub async fn start(&self, config_path: &str) -> Result<(), AppError> {
        let mut status = self.status.lock().map_err(|e| AppError::Sidecar(e.to_string()))?;
        if *status == SidecarStatus::Running {
            info!("sing-box 已在运行，跳过启动");
            return Ok(());
        }
        *status = SidecarStatus::Starting;
        drop(status);

        let binary_path = Self::resolve_binary_path()?;
        if !binary_path.exists() {
            let err_msg = format!("找不到 sing-box 二进制文件: {:?}", binary_path);
            error!("{}", err_msg);
            *self.status.lock().map_err(|e| AppError::Sidecar(e.to_string()))? = SidecarStatus::Error(err_msg.clone());
            return Err(AppError::Sidecar(err_msg));
        }

        info!("启动 sing-box: {:?} run -c {}", binary_path, config_path);

        let child = tokio::process::Command::new(&binary_path)
            .arg("run")
            .arg("-c")
            .arg(config_path)
            .spawn()
            .map_err(|e| AppError::Sidecar(format!("拉起 sing-box 失败: {}", e)))?;

        let mut proc_guard = self.process.lock().map_err(|e| AppError::Sidecar(e.to_string()))?;
        *proc_guard = Some(child);

        *self.status.lock().map_err(|e| AppError::Sidecar(e.to_string()))? = SidecarStatus::Running;

        info!("sing-box 子进程启动成功");
        Ok(())
    }

    /// 停止 sing-box 进程
    pub async fn stop(&self) -> Result<(), AppError> {
        let mut proc = self.process.lock().map_err(|e| AppError::Sidecar(e.to_string()))?;
        if let Some(mut child) = proc.take() {
            if let Err(e) = child.kill().await {
                warn!("停止 sing-box 进程时出现警告: {}", e);
            }
            info!("sing-box 进程已停止");
        }
        *self.status.lock().map_err(|e| AppError::Sidecar(e.to_string()))? = SidecarStatus::Stopped;
        Ok(())
    }

    /// 获取当前状态
    pub fn get_status(&self) -> SidecarStatus {
        self.status.lock()
            .map(|s| s.clone())
            .unwrap_or(SidecarStatus::Error("锁获取失败".to_string()))
    }

    /// 系统唤醒后重启（处理睡眠/唤醒断连）
    pub async fn handle_wake(&self, config_path: &str) -> Result<(), AppError> {
        warn!("系统从睡眠唤醒，重启 sing-box...");
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start(config_path).await
    }

    /// 解析当前平台对应的 sing-box 二进制路径
    fn resolve_binary_path() -> Result<PathBuf, AppError> {
        #[cfg(target_os = "windows")]
        let rel_path = format!("src-tauri/sidecar-bin/windows-x64/sing-box-{}.exe", SINGBOX_VERSION);
        #[cfg(target_os = "macos")]
        let rel_path = format!("src-tauri/sidecar-bin/macos-universal/sing-box-{}", SINGBOX_VERSION);
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let rel_path = format!("src-tauri/sidecar-bin/sing-box-{}", SINGBOX_VERSION);

        Ok(PathBuf::from(rel_path))
    }
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}
