#![cfg(target_os = "windows")]
/// Windows 具名管道 IPC 服务端
/// 作者: TanXiang
///
/// 运行在 AuroDaemon 系统服务/计划任务进程内，通过具名管道 `\\.\pipe\Auroweave.Core.Control`
/// 接收主程序（Tauri 前端）发送的控制指令。
///
/// IPC 服务流程：
/// 1. **管道创建**：使用 SDDL 安全描述符创建管道，允许 SYSTEM/Administrators 完全控制，
///    Authenticated Users 读写访问；单管道实例上限 4 个，防止恶意客户端耗尽句柄
/// 2. **连接等待**：循环创建管道实例并等待客户端连接
/// 3. **请求处理**：每个客户端连接 spawn 独立任务处理，支持并发请求
/// 4. **Token 校验**：以常数时间比较验证请求中的 Token 与安装时生成的明文 Token 是否匹配
/// 5. **指令分发**：根据 action 字段分发到 CoreManager 的对应方法
///
/// **分帧协议（长度前缀）**：
/// 请求不再是一次性单次 read（旧实现对超过 64KB 的大配置会截断），改为：
/// - 客户端先发送 8 字节 little-endian u64 长度头（JSON 体长度），随后紧跟 JSON 体
/// - 服务端先循环 read 读满 8 字节长度头，再按长度分配 Vec 循环 read 读满
/// - 长度上限 16MB（MAX_FRAME_LEN），超出视为滥用直接断开
/// - 主程序 ipc_client.rs 已同步实现该分帧协议（协调标记）
///
/// 支持的指令：
/// - `GET_STATUS`：查询内核运行状态和 PID
/// - `RELOAD_CONFIG`：重载配置并重启内核（config 仅接受内嵌配置文本）
/// - `SHUTDOWN_CORE`：停止内核进程
///
/// 安全设计：
/// - 管道使用 SDDL 限制访问权限，仅允许已认证用户连接
/// - 每个请求必须携带正确的 Token，防止未授权进程发送指令
/// - Token 为明文随机 UUID v4，存储于仅 SYSTEM/Admin 可访问的文件中
///   （依赖文件 ACL 保护，不再使用硬编码共享密钥加密——加密无意义）
/// - Token 比较使用常数时间比较，避免时序侧信道泄漏匹配前缀长度
/// - Token 校验失败与参数错误统一返回"身份验证失败"，不区分细节，避免信息泄漏
/// - RELOAD_CONFIG 的 config 参数一律作为配置文本处理，绝不作为文件路径读取
///   （历史版本曾支持路径自适应读取，构成 SYSTEM 权限任意文件读取原语，已删除）
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::NamedPipeServer;
use tracing::{error, info, warn};
use crate::core_manager::{CoreManager, CoreStatus};

const PIPE_NAME: &str = r"\\.\pipe\Auroweave.Core.Control";

/// 单个请求帧（JSON 体）的最大允许长度，超出视为滥用直接断开
const MAX_FRAME_LEN: u64 = 16 * 1024 * 1024;

/// 管道实例上限：限制单机可同时创建的管道实例数量，防止恶意客户端
/// 反复连接耗尽服务端句柄/内存（原实现为 PIPE_UNLIMITED_INSTANCES）
const MAX_PIPE_INSTANCES: u32 = 4;

#[derive(Debug, serde::Deserialize)]
pub struct IpcRequest {
    /// 操作指令: "GET_STATUS" | "RELOAD_CONFIG" | "SHUTDOWN_CORE"
    pub action: String,
    /// 安全令牌（明文，用于身份验证，常数时间比较）
    pub token: String,
    /// 可选的配置内容（仅 RELOAD_CONFIG 使用；一律作为配置文本处理，绝不作为文件路径）
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
        let token = match Self::load_token() {
            Ok(t) if !t.is_empty() => t,
            Ok(_) => {
                // Token 文件存在但为空——视为无效，拒绝所有请求并留痕
                error!("IPC Token 文件内容为空，服务将以拒绝所有请求模式运行");
                String::new()
            }
            Err(e) => {
                // Token 加载失败不静默：记录错误日志后继续启动（空 Token 会使
                // 后续所有请求校验必然失败），保证攻击行为至少在日志中留痕
                error!("IPC Token 加载失败: {}，服务将以拒绝所有请求模式运行", e);
                String::new()
            }
        };
        Self { core_manager, token }
    }

    /// 启动 IPC 服务端并阻塞运行，直到收到关闭信号
    ///
    /// 运行流程：
    /// 1. 循环创建安全具名管道实例（上限 MAX_PIPE_INSTANCES）
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
    /// 1. 按长度前缀分帧协议读取请求数据（8 字节 LE u64 长度头 + JSON 体）
    /// 2. 校验 Token（空或不匹配则拒绝，常数时间比较）
    /// 3. 根据 action 分发指令到 CoreManager
    /// 4. 序列化响应并写回管道
    async fn handle_client(&self, mut server: NamedPipeServer) -> Result<(), String> {
        // ---- 步骤1: 按分帧协议读取请求 ----
        let request: IpcRequest = match self.read_framed_request(&mut server).await {
            Ok(req) => req,
            Err(e) => {
                let resp = IpcResponse {
                    success: false,
                    status: "error".to_string(),
                    error: Some("身份验证失败".to_string()),
                    pid: None,
                };
                let _ = server.write_all(&serde_json::to_vec(&resp).unwrap_or_default()).await;
                // 协议层错误（超长帧/连接中断）仅记服务端日志，不给客户端区分细节
                error!("读取/解析 IPC 请求失败: {}", e);
                return Err(e);
            }
        };

        info!("收到客户端 IPC 请求: action={}", request.action);

        // ---- 步骤2: 校验安全 Token ----
        // Token 为空或不匹配则拒绝请求，防止未授权进程控制内核；
        // 常数时间比较避免时序侧信道泄漏匹配前缀长度
        if self.token.is_empty() || !constant_time_eq(request.token.as_bytes(), self.token.as_bytes()) {
            let resp = IpcResponse {
                success: false,
                status: "error".to_string(),
                error: Some("身份验证失败".to_string()),
                pid: None,
            };
            let _ = server.write_all(&serde_json::to_vec(&resp).unwrap_or_default()).await;
            error!("客户端安全 Token 校验失败！");
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
                // config 参数一律作为内嵌配置文本处理（分帧协议已支持大配置传输）。
                // 安全说明：历史版本曾支持"若为存在路径则由 SYSTEM 服务直接读取该文件"，
                // 构成任意文件读取原语，该分支已删除。
                match request.config {
                    Some(config_content) if !config_content.is_empty() => {
                        info!("以文本形式接收配置参数，长度: {}", config_content.len());
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
                    }
                    _ => {
                        error!("重载配置失败，参数为空");
                        response.success = false;
                        response.status = "error".to_string();
                        response.error = Some("配置内容不能为空".to_string());
                    }
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
                response.error = Some("未知的操作指令".to_string());
            }
        }

        let resp_bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
        server.write_all(&resp_bytes).await.map_err(|e| e.to_string())?;
        let _ = server.flush().await;

        Ok(())
    }

    /// 按长度前缀分帧协议读取并解析一条 IPC 请求
    ///
    /// 协议：8 字节 little-endian u64 长度头 + JSON 体。
    /// - 长度头需循环 read 读满 8 字节（管道是字节流，单次 read 不保证读满）
    /// - JSON 体按长度分配 Vec 后循环 read 读满
    /// - 长度超过 MAX_FRAME_LEN（16MB）视为滥用，拒绝处理
    async fn read_framed_request(&self, server: &mut NamedPipeServer) -> Result<IpcRequest, String> {
        // 读满 8 字节长度头
        let mut len_buf = [0u8; 8];
        read_exact_or_eof(server, &mut len_buf).await?;
        let frame_len = u64::from_le_bytes(len_buf);

        if frame_len > MAX_FRAME_LEN {
            return Err(format!("请求帧长度超限 ({} > {})", frame_len, MAX_FRAME_LEN));
        }
        if frame_len == 0 {
            return Err("请求帧长度为 0".to_string());
        }

        // 按长度读满 JSON 体
        let mut body = vec![0u8; frame_len as usize];
        read_exact_or_eof(server, &mut body).await?;

        serde_json::from_slice(&body).map_err(|e| format!("JSON 解析失败: {}", e))
    }

    /// 加载 IPC 安全令牌（明文）
    ///
    /// 流程：
    /// 1. 读取 `%ProgramData%\Auroweave\data\ipc_token.bin`
    /// 2. 内容为明文 token 字符串（依赖文件 ACL 保护，仅 SYSTEM/Admin 可访问；
    ///    不再使用硬编码共享密钥的 AES-GCM 解密——该密钥同时编译进多个二进制，
    ///    加密不提供任何真实安全性）
    /// 3. 返回 trim 后的 token 字符串
    fn load_token() -> Result<String, String> {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        let token_path = std::path::PathBuf::from(program_data).join("Auroweave").join("data").join("ipc_token.bin");
        if !token_path.exists() {
            return Err("Token 文件不存在".to_string());
        }
        let content = std::fs::read(&token_path).map_err(|e| e.to_string())?;
        let token_str = String::from_utf8(content).map_err(|_| "Token UTF-8 解析失败".to_string())?;
        Ok(token_str.trim().to_string())
    }
}

/// 循环 read 直到读满 buf 或对端关闭（读到 0 字节且一无所获即 EOF）
///
/// 字节流管道上单次 `read` 可能返回少于缓冲区长度的数据，
/// 长度头和 JSON 体都必须循环读取直至凑满。
async fn read_exact_or_eof(server: &mut NamedPipeServer, buf: &mut [u8]) -> Result<(), String> {
    let mut filled = 0usize;
    while filled < buf.len() {
        let n = server.read(&mut buf[filled..]).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("管道在读取完成前被客户端关闭".to_string());
        }
        filled += n;
    }
    Ok(())
}

/// 常数时间字节比较（避免短路比较带来的时序侧信道）
///
/// 逐字节 XOR 后 OR 累积差异，长度差异直接判为不等；
/// 整体耗时与数据长度相关、与匹配前缀长度无关，不泄漏信息。
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
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
/// 实例上限: MAX_PIPE_INSTANCES（4），防止单机句柄/内存耗尽
fn create_secure_named_pipe(pipe_name: &str) -> Result<NamedPipeServer, String> {
    use windows_sys::Win32::System::Pipes::{CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_READMODE_BYTE, PIPE_WAIT};
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
            // 实例数上限 4：不再使用 PIPE_UNLIMITED_INSTANCES，防句柄耗尽
            MAX_PIPE_INSTANCES,
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
