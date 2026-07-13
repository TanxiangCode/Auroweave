/// sing-box 子进程管理
/// 作者: TanXiang
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub const SINGBOX_VERSION: &str = "1.13.14";

#[derive(Debug, Clone, PartialEq)]
pub enum CoreStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

pub struct CoreManager {
    process: Arc<Mutex<Option<Child>>>,
    status: Arc<Mutex<CoreStatus>>,
}

impl CoreManager {
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(CoreStatus::Stopped)),
        }
    }

    pub async fn get_status(&self) -> (CoreStatus, Option<u32>) {
        let status = self.status.lock().await.clone();
        let pid = if let Some(ref child) = *self.process.lock().await {
            child.id()
        } else {
            None
        };
        (status, pid)
    }

    pub async fn start(&self, config_content: &str) -> Result<(), String> {
        let mut status_guard = self.status.lock().await;
        if *status_guard == CoreStatus::Running {
            info!("sing-box 已经在运行，准备重启...");
            drop(status_guard);
            self.stop().await?;
            status_guard = self.status.lock().await;
        }

        *status_guard = CoreStatus::Starting;

        // 获取可执行文件路径
        let binary_path = match Self::resolve_binary_path() {
            Ok(p) => p,
            Err(e) => {
                let err_msg = format!("解析 sing-box 二进制路径失败: {}", e);
                *status_guard = CoreStatus::Error(err_msg.clone());
                return Err(err_msg);
            }
        };

        info!("找到 sing-box 二进制文件: {:?}", binary_path);

        // 写入配置缓存文件
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        let cache_dir = PathBuf::from(program_data).join("Auroweave");
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            let err_msg = format!("创建服务缓存目录失败: {}", e);
            *status_guard = CoreStatus::Error(err_msg.clone());
            return Err(err_msg);
        }
        let config_path = cache_dir.join("config.json");
        if let Err(e) = std::fs::write(&config_path, config_content) {
            let err_msg = format!("写入配置文件失败: {}", e);
            *status_guard = CoreStatus::Error(err_msg.clone());
            return Err(err_msg);
        }

        info!("启动 sing-box: {:?} run -c {:?}", binary_path, config_path);

        use std::process::Stdio;
        let mut child = match tokio::process::Command::new(&binary_path)
            .arg("run")
            .arg("-c")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("拉起 sing-box 进程失败: {}", e);
                *status_guard = CoreStatus::Error(err_msg.clone());
                return Err(err_msg);
            }
        };

        // 取出 stdout/stderr 管道输出日志
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stderr_lines_arc = Arc::new(Mutex::new(Vec::<String>::new()));
        if let Some(stderr_stream) = stderr {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let arc_clone = stderr_lines_arc.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr_stream).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    error!("[sing-box error] {}", line);
                    arc_clone.lock().await.push(line);
                }
            });
        }

        if let Some(stdout_stream) = stdout {
            use tokio::io::{AsyncBufReadExt, BufReader};
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout_stream).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    info!("[sing-box] {}", line);
                }
            });
        }

        // 等待 1.5 秒，检测进程是否因配置错误或权限不足提前退出
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

        match child.try_wait() {
            Ok(Some(exit_status)) => {
                let mut err_msg = format!("sing-box 启动后立即退出，退出码: {:?}", exit_status.code());
                let lines = stderr_lines_arc.lock().await;
                if !lines.is_empty() {
                    err_msg = format!("{}\n详情: {}", err_msg, lines.join("\n"));
                }
                *status_guard = CoreStatus::Stopped;
                return Err(err_msg);
            }
            Ok(None) => {
                // 仍在运行
            }
            Err(e) => {
                warn!("检查进程状态错误: {}", e);
            }
        }

        let process_clone = self.process.clone();
        let status_clone = self.status.clone();
        
        // 开启监听进程退出的任务
        *self.process.lock().await = Some(child);
        *status_guard = CoreStatus::Running;
        
        tokio::spawn(async move {
            let mut p_guard = process_clone.lock().await;
            if let Some(ref mut child) = *p_guard {
                match child.wait().await {
                    Ok(status) => {
                        info!("sing-box 进程退出，状态为: {:?}", status);
                    }
                    Err(e) => {
                        error!("监听进程退出时发生异常: {}", e);
                    }
                }
            }
            *process_clone.lock().await = None;
            *status_clone.lock().await = CoreStatus::Stopped;
        });

        info!("sing-box 托管进程启动成功");
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), String> {
        let mut proc_guard = self.process.lock().await;
        if let Some(mut child) = proc_guard.take() {
            info!("正在停止 sing-box 进程...");
            if let Err(e) = child.kill().await {
                warn!("杀死 sing-box 进程警告: {}", e);
            }
            // 等待退出
            let _ = child.wait().await;
            info!("sing-box 进程已终止");
        }
        *self.status.lock().await = CoreStatus::Stopped;
        Ok(())
    }

    fn resolve_binary_path() -> Result<PathBuf, String> {
        // 服务是以其自身所在的 Program Files 目录为基准寻找同级目录下的 sing-box 二进制
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // 1. 尝试寻找当前 exe 所在目录下的 sing-box-1.13.14.exe
                let target = exe_dir.join(format!("sing-box-{}.exe", SINGBOX_VERSION));
                if target.exists() {
                    return Ok(target);
                }
                // 2. 尝试寻找当前 exe 所在目录下的 sing-box.exe
                let target_generic = exe_dir.join("sing-box.exe");
                if target_generic.exists() {
                    return Ok(target_generic);
                }
                // 3. 尝试寻找 sidecar 候选路径（便于开发调试服务）
                let debug_path = exe_dir.join(format!("../../../src-tauri/sidecar-bin/windows-x64/sing-box-{}.exe", SINGBOX_VERSION));
                if debug_path.exists() {
                    return Ok(debug_path);
                }
            }
        }
        Err("在服务所在目录下找不到任何 sing-box 可执行二进制文件".to_string())
    }
}
