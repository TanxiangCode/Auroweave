/// sing-box sidecar 生命周期管理
/// 作者: TanXiang
use crate::error::AppError;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::process::Child;
use tracing::{info, warn, error};

pub const SINGBOX_VERSION: &str = "1.13.14";

#[derive(Debug, Clone, PartialEq)]
pub enum SidecarStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

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

    pub async fn start(&self, config_path: &str) -> Result<(), AppError> {
        let mut status = self.status.lock().map_err(|e| AppError::Sidecar(e.to_string()))?;
        if *status == SidecarStatus::Running {
            info!("sing-box 已在运行，跳过启动");
            return Ok(());
        }
        *status = SidecarStatus::Starting;
        drop(status);

        let binary_path = Self::resolve_binary_path()?;
        info!("找到 sing-box 执行文件: {:?}", binary_path);
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

    pub async fn stop(&self) -> Result<(), AppError> {
        let child = {
            let mut proc = self.process.lock().map_err(|e| AppError::Sidecar(e.to_string()))?;
            proc.take()
        };

        if let Some(mut child) = child {
            if let Err(e) = child.kill().await {
                warn!("停止 sing-box 进程时出现警告: {}", e);
            }
            info!("sing-box 进程已停止");
        }
        *self.status.lock().map_err(|e| AppError::Sidecar(e.to_string()))? = SidecarStatus::Stopped;
        Ok(())
    }

    pub fn get_status(&self) -> SidecarStatus {
        self.status.lock()
            .map(|s| s.clone())
            .unwrap_or(SidecarStatus::Error("锁获取失败".to_string()))
    }

    pub async fn handle_wake(&self, config_path: &str) -> Result<(), AppError> {
        warn!("系统从睡眠唤醒，重启 sing-box...");
        self.stop().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        self.start(config_path).await
    }

    /// 多路径候选自动匹配算法（防止工作目录或解压版本引起的找不到路径）
    fn resolve_binary_path() -> Result<PathBuf, AppError> {
        let mut candidates = Vec::new();

        #[cfg(target_os = "windows")]
        {
            candidates.push(format!("src-tauri/sidecar-bin/windows-x64/sing-box-{}.exe", SINGBOX_VERSION));
            candidates.push(format!("sidecar-bin/windows-x64/sing-box-{}.exe", SINGBOX_VERSION));
            candidates.push("src-tauri/sidecar-bin/windows-x64/sing-box-1.11.4.exe".to_string());
            candidates.push("sidecar-bin/windows-x64/sing-box-1.11.4.exe".to_string());
        }

        #[cfg(target_os = "macos")]
        {
            candidates.push(format!("src-tauri/sidecar-bin/macos-universal/sing-box-{}", SINGBOX_VERSION));
            candidates.push(format!("sidecar-bin/macos-universal/sing-box-{}", SINGBOX_VERSION));
            candidates.push("src-tauri/sidecar-bin/macos-universal/sing-box-1.11.4".to_string());
            candidates.push("sidecar-bin/macos-universal/sing-box-1.11.4".to_string());
        }

        // 检查程序运行路径（针对生产构建产物）
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                #[cfg(target_os = "windows")]
                candidates.push(exe_dir.join(format!("sing-box-{}.exe", SINGBOX_VERSION)).to_string_lossy().to_string());
                #[cfg(target_os = "macos")]
                candidates.push(exe_dir.join(format!("sing-box-{}", SINGBOX_VERSION)).to_string_lossy().to_string());
            }
        }

        for path_str in &candidates {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Ok(path);
            }
        }

        let err_msg = format!(
            "找不到 sing-box 二进制文件 (尝试路径: {:?})，请运行 download-sidecar 脚本",
            candidates
        );
        error!("{}", err_msg);
        Err(AppError::Sidecar(err_msg))
    }
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}
