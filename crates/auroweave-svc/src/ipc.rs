#![cfg(target_os = "windows")]
/// Windows 具名管道 IPC 服务端
/// 作者: TanXiang
///
/// 运行在 AuroDaemon 系统服务/计划任务进程内，通过具名管道 `\\.\pipe\Auroweave.Core.Control`
/// 接收主程序（Tauri 前端）发送的控制指令。
///
/// IPC 服务流程：
/// 1. **管道创建**：使用 SDDL 安全描述符创建管道，允许 SYSTEM/Administrators 完全控制，
///    Authenticated Users 读写访问
/// 2. **连接等待**：循环创建管道实例并等待客户端连接
/// 3. **请求处理**：每个客户端连接 spawn 独立任务处理，支持并发请求
/// 4. **Token 校验**：验证请求中的 Token 与安装时生成的加密 Token 是否匹配
/// 5. **指令分发**：根据 action 字段分发到 CoreManager 的对应方法
///
/// 支持的指令：
/// - `GET_STATUS`：查询内核运行状态和 PID
/// - `RELOAD_CONFIG`：重载配置并重启内核（config 可为文件路径或配置文本）
/// - `SHUTDOWN_CORE`：停止内核进程
///
/// 安全设计：
/// - 管道使用 SDDL 限制访问权限，仅允许已认证用户连接
/// - 每个请求必须携带正确的 Token，防止未授权进程发送指令
/// - Token 使用 AES-256-GCM 加密存储，密钥硬编码在二进制中
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::NamedPipeServer;
use tracing::{error, info, warn};
use crate::core_manager::{CoreManager, CoreStatus};

const PIPE_NAME: &str = r"\\.\pipe\Auroweave.Core.Control";

#[derive(Debug, serde::Deserialize)]
pub struct IpcRequest {
    /// 操作指令: "GET_STATUS" | "RELOAD_CONFIG" | "SHUTDOWN_CORE"
    pub action: String,
    /// 安全令牌（AES-256-GCM 解密后的明文，用于身份验证）
    pub token: String,
    /// 可选的配置内容或配置文件路径（仅 RELOAD_CONFIG 使用）
    pub config: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct IpcResponse {
    pub success: bool,
    pub status: String,
    pub error: Option<String>,
    pub pid: Option<u32>,
}

pub struct IpcServer {
    core_manager: Arc<CoreManager>,
    token: String,
}

impl IpcServer {
    pub fn new(core_manager: Arc<CoreManager>) -> Self {
        let token = Self::load_token().unwrap_or_default();
        Self { core_manager, token }
    }

    /// 启动 IPC 服务端并阻塞运行，直到收到关闭信号
    ///
    /// 运行流程：
    /// 1. 循环创建安全具名管道实例
    /// 2. 使用 `tokio::select!` 同时等待客户端连接和关闭信号
    /// 3. 客户端连接后 spawn 独立任务处理请求
    /// 4. 收到关闭信号时退出循环
    pub async fn run(self: Arc<Self>, mut shutdown_rx: tokio::sync::oneshot::Receiver<()>) {
        info!("IPC 服务端开始运行，管道地址: {}", PIPE_NAME);

        loop {
            let server = match create_secure_named_pipe(PIPE_NAME) {
                Ok(s) => s,
                Err(e) => {
                    error!("创建具名管道失败: {}，1秒后重试...", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            // 等待客户端连接或收到关闭信号
            tokio::select! {
                res = server.connect() => {
                    match res {
                        Ok(_) => {
                            let self_clone = self.clone();
                            tokio::spawn(async move {
                                if let Err(e) = self_clone.handle_client(server).await {
                                    warn!("处理 IPC 客户端请求失败: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            warn!("具名管道客户端连接失败: {}", e);
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    info!("收到退出信号，停止 IPC 服务监听");
                    break;
                }
            }
        }
    }

    /// 处理单个客户端连接
    ///
    /// 处理流程：
    /// 1. 读取请求数据并反序列化为 `IpcRequest`
    /// 2. 校验 Token（空或不匹配则拒绝）
    /// 3. 根据 action 分发指令到 CoreManager
    /// 4. 序列化响应并写回管道
    async fn handle_client(&self, mut server: NamedPipeServer) -> Result<(), String> {
        // ---- 步骤1: 读取并解析请求数据 ----
        let mut buffer = vec![0u8; 65536];
        let n = server.read(&mut buffer).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }

        let request: IpcRequest = match serde_json::from_slice(&buffer[..n]) {
            Ok(req) => req,
            Err(e) => {
                let resp = IpcResponse {
                    success: false,
                    status: "error".to_string(),
                    error: Some(format!("JSON 解析失败: {}", e)),
                    pid: None,
                };
                let _ = server.write_all(&serde_json::to_vec(&resp).unwrap()).await;
                error!("解析请求 JSON 失败: {}", e);
                return Err(format!("解析请求 JSON 失败: {}", e));
            }
        };

        info!("收到客户端 IPC 请求: action={}", request.action);

        // ---- 步骤2: 校验安全 Token ----
        // Token 为空或不匹配则拒绝请求，防止未授权进程控制内核
        if request.token.is_empty() || request.token != self.token {
            let resp = IpcResponse {
                success: false,
                status: "error".to_string(),
                error: Some("身份验证失败，Token 不匹配".to_string()),
                pid: None,
            };
            let _ = server.write_all(&serde_json::to_vec(&resp).unwrap()).await;
            error!("客户端安全 Token 校验失败，Token 不匹配！");
            return Err("客户端 Token 校验未通过".to_string());
        }

        let mut response = IpcResponse {
            success: true,
            status: "stopped".to_string(),
            error: None,
            pid: None,
        };

        // ---- 步骤3: 根据 action 分发指令 ----
        match request.action.as_str() {
            "GET_STATUS" => {
                let (status, pid) = self.core_manager.get_status().await;
                response.status = status_to_str(status);
                response.pid = pid;
                info!("IPC GET_STATUS: status={}, pid={:?}", response.status, response.pid);
            }
            "RELOAD_CONFIG" => {
                // 重载配置并重启内核
                // 自适应判断：config 参数可能是文件路径或配置文本内容
                // 若为文件路径则服务端直接读取，避开 IPC 管道 64KB 缓冲区的大文件截断问题
                if let Some(config_param) = request.config {
                    // 自适应判断：如果参数是一个物理文件路径则读取其内容，避开 IPC 管道大文件分包截断
                    let config_content = if std::path::Path::new(&config_param).exists() {
                        info!("检测到物理配置文件路径: {:?}", config_param);
                        match std::fs::read_to_string(&config_param) {
                            Ok(content) => {
                                info!("成功读取配置文件内容，长度: {}", content.len());
                                content
                            }
                            Err(e) => {
                                error!("系统服务读取配置文件失败: {}", e);
                                response.success = false;
                                response.status = "error".to_string();
                                response.error = Some(format!("系统服务读取配置文件失败: {}", e));
                                let resp_bytes = serde_json::to_vec(&response).unwrap();
                                let _ = server.write_all(&resp_bytes).await;
                                return Err(format!("系统服务读取配置文件失败: {}", e));
                            }
                        }
                    } else {
                        info!("直接以文本形式接收配置参数，长度: {}", config_param.len());
                        config_param
                    };

                    match self.core_manager.start(&config_content).await {
                        Ok(_) => {
                            let (status, pid) = self.core_manager.get_status().await;
                            response.status = status_to_str(status);
                            response.pid = pid;
                            info!("重载内核配置并成功拉起，当前状态: {}, PID: {:?}", response.status, response.pid);
                        }
                        Err(e) => {
                            error!("启动 sing-box 失败: {}", e);
                            response.success = false;
                            response.status = "error".to_string();
                            response.error = Some(e);
                        }
                    }
                } else {
                    error!("重载配置失败，参数为空");
                    response.success = false;
                    response.status = "error".to_string();
                    response.error = Some("配置内容或路径不能为空".to_string());
                }
            }
            "SHUTDOWN_CORE" => {
                info!("请求停止内核服务...");
                if let Err(e) = self.core_manager.stop().await {
                    error!("停止内核核心服务失败: {}", e);
                    response.success = false;
                    response.status = "error".to_string();
                    response.error = Some(e);
                } else {
                    response.status = "stopped".to_string();
                    info!("内核服务停止成功");
                }
            }
            _ => {
                error!("未知指令 action: {}", request.action);
                response.success = false;
                response.status = "error".to_string();
                response.error = Some(format!("未知的操作指令: {}", request.action));
            }
        }

        let resp_bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
        server.write_all(&resp_bytes).await.map_err(|e| e.to_string())?;
        let _ = server.flush().await;

        Ok(())
    }

    /// 加载并解密 IPC 安全令牌
    ///
    /// 解密流程：
    /// 1. 读取 `%ProgramData%\Auroweave\data\ipc_token.bin` 加密文件
    /// 2. 前 12 字节为 AES-GCM Nonce，剩余部分为密文
    /// 3. 使用硬编码的 AES-256 密钥解密，得到明文 Token
    fn load_token() -> Result<String, String> {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        let token_path = std::path::PathBuf::from(program_data).join("Auroweave").join("data").join("ipc_token.bin");
        if !token_path.exists() {
            return Err("Token 文件不存在".to_string());
        }
        let content = std::fs::read(token_path).map_err(|e| e.to_string())?;
        
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };
        const TOKEN_KEY: &[u8; 32] = b"AuroweaveIPCSecretKey2026_Secure";
        
        if content.len() < 12 {
            return Err("Token 文件已损坏".to_string());
        }
        
        let key: &aes_gcm::Key<Aes256Gcm> = TOKEN_KEY.into();
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::try_from(&content[..12]).unwrap();
        let ciphertext = &content[12..];
        
        let plaintext = cipher.decrypt(&nonce, ciphertext)
            .map_err(|_| "Token 解密失败".to_string())?;
            
        let token_str = String::from_utf8(plaintext)
            .map_err(|_| "Token UTF-8 解析失败".to_string())?;
            
        Ok(token_str.trim().to_string())
    }
}

/// 将 CoreStatus 枚举转换为字符串状态码
///
/// 用于 IPC 响应中的 status 字段: stopped / starting / running / error
fn status_to_str(status: CoreStatus) -> String {
    match status {
        CoreStatus::Stopped => "stopped".to_string(),
        CoreStatus::Starting => "starting".to_string(),
        CoreStatus::Running => "running".to_string(),
        CoreStatus::Error(_) => "error".to_string(),
    }
}

/// 创建安全具名管道
///
/// 使用 SDDL 安全描述符创建管道，权限分配：
/// - SYSTEM (SY): 完全控制
/// - Administrators (BA): 完全控制
/// - Authenticated Users (AU): 读写访问
///
/// 管道模式: 双工 + 字节流 + OVERLAPPED 异步 I/O
fn create_secure_named_pipe(pipe_name: &str) -> Result<NamedPipeServer, String> {
    use windows_sys::Win32::System::Pipes::{CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_READMODE_BYTE, PIPE_WAIT, PIPE_UNLIMITED_INSTANCES};
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED;
    use windows_sys::Win32::Security::{SECURITY_ATTRIBUTES, PSECURITY_DESCRIPTOR};
    use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
    use windows_sys::Win32::Foundation::{LocalFree, INVALID_HANDLE_VALUE};

    const PIPE_ACCESS_DUPLEX: u32 = 3;

    let pipe_name_w: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
    
    // SDDL: D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GWGR;;;AU)
    // SYSTEM(SY) 和 Administrators(BA) 拥有全部权限，Authenticated Users(AU) 拥有读写访问权
    let sddl = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GWGR;;;AU)";
    let sddl_w: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_w.as_ptr(),
            1, // SDDL_REVISION_1
            &mut sd,
            std::ptr::null_mut(),
        ) == 0 {
            return Err("SECURITY_DESCRIPTOR 转换失败".to_string());
        }

        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };

        let open_mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
        let pipe_mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT;

        let handle = CreateNamedPipeW(
            pipe_name_w.as_ptr(),
            open_mode,
            pipe_mode,
            PIPE_UNLIMITED_INSTANCES,
            65536,
            65536,
            0,
            &sa,
        );

        LocalFree(sd);

        if handle == INVALID_HANDLE_VALUE {
            return Err("调用 CreateNamedPipeW 失败".to_string());
        }

        let server = NamedPipeServer::from_raw_handle(handle as *mut std::ffi::c_void)
            .map_err(|e| format!("具名管道句柄转换失败: {}", e))?;
        Ok(server)
    }
}
