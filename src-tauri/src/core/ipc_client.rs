/// Windows 系统服务 IPC 具名管道客户端
/// 作者: TanXiang
///
/// 职责：主程序（Tauri 前端进程）通过具名管道向 AuroDaemon 系统服务发送控制指令。
///
/// IPC 通信流程：
/// 1. **加载安全令牌**：从 `%ProgramData%\Auroweave\data\ipc_token.bin` 读取并解密 Token
///    - Token 使用 AES-256-GCM 加密存储，防止篡改和伪造
///    - 安装时由 `installer::setup_token` 生成并加密写入
/// 2. **连接管道**：通过 `\\.\pipe\Auroweave.Core.Control` 连接服务端
///    - 若连接失败说明服务未运行，返回错误提示
/// 3. **发送请求**：序列化 `IpcRequest`（action + token + config）写入管道
/// 4. **接收响应**：读取管道返回数据，反序列化为 `IpcResponse`
///
/// 支持的 action：
/// - `GET_STATUS`：查询 sing-box 内核运行状态和 PID
/// - `RELOAD_CONFIG`：重载配置并重启内核（config 参数可为文件路径或配置文本）
/// - `SHUTDOWN_CORE`：停止内核进程
use serde::{Deserialize, Serialize};

const PIPE_NAME: &str = r"\\.\pipe\Auroweave.Core.Control";

/// IPC 请求结构体
/// - `action`: 操作指令 (GET_STATUS / RELOAD_CONFIG / SHUTDOWN_CORE)
/// - `token`: 安全令牌（AES-256-GCM 解密后的明文）
/// - `config`: 可选的配置内容或配置文件路径（仅 RELOAD_CONFIG 使用）
#[derive(Debug, Serialize)]
struct IpcRequest {
    action: String,
    token: String,
    config: Option<String>,
}

/// IPC 响应结构体
/// - `success`: 操作是否成功
/// - `status`: 内核当前状态 (stopped / starting / running / error)
/// - `error`: 失败时的错误描述
/// - `pid`: sing-box 进程 PID（运行中时有值）
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IpcResponse {
    pub success: bool,
    pub status: String,
    pub error: Option<String>,
    pub pid: Option<u32>,
}

#[cfg(target_os = "windows")]
pub async fn send_ipc_request(action: &str, config_content: Option<&str>) -> Result<IpcResponse, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    log::info!("[ipc_client] 准备发送 IPC 请求: action={}, 参数长度={}", action, config_content.map(|c| c.len()).unwrap_or(0));

    // ---- 步骤1: 加载安全令牌 ----
    // 从加密文件中读取并解密 Token，用于服务端身份验证
    let token = match load_token() {
        Ok(t) => t,
        Err(e) => {
            log::error!("[ipc_client] 获取安全令牌 Token 失败: {}", e);
            return Err(format!("本地服务验证 Token 获取失败: {}", e));
        }
    };

    // ---- 步骤2: 连接服务管道 ----
    // 若连接失败，通常意味着系统服务未启动或未安装
    let mut client = match tokio::net::windows::named_pipe::ClientOptions::new().open(PIPE_NAME) {
        Ok(c) => c,
        Err(e) => {
            log::error!("[ipc_client] 无法打开具名管道连接: {}", e);
            return Err(format!("无法连接至服务控制管道 (可能服务未运行): {}", e));
        }
    };

    // ---- 步骤3: 构建并发送请求 ----
    // 序列化 IpcRequest 为 JSON 并写入管道
    let req = IpcRequest {
        action: action.to_string(),
        token,
        config: config_content.map(|s| s.to_string()),
    };

    let req_bytes = serde_json::to_vec(&req).map_err(|e| format!("序列化请求失败: {}", e))?;
    
    client.write_all(&req_bytes).await.map_err(|e| format!("向管道写入指令失败: {}", e))?;
    client.flush().await.map_err(|e| format!("清空管道缓冲区失败: {}", e))?;

    // ---- 步骤4: 接收并解析响应 ----
    // 读取服务端返回的数据（最大 64KB），反序列化为 IpcResponse
    let mut buf = vec![0u8; 65536];
    let n = client.read(&mut buf).await.map_err(|e| format!("读取管道响应失败: {}", e))?;
    if n == 0 {
        log::error!("[ipc_client] 管道已断开且无返回数据");
        return Err("服务未返回任何响应 data".to_string());
    }

    let resp: IpcResponse = serde_json::from_slice(&buf[..n])
        .map_err(|e| format!("解析管道返回数据失败: {}", e))?;

    log::info!("[ipc_client] 收到 IPC 管道反馈: success={}, status={}", resp.success, resp.status);
    Ok(resp)
}

/// 加载并解密 IPC 安全令牌
///
/// 解密流程：
/// 1. 读取 `%ProgramData%\Auroweave\data\ipc_token.bin` 加密文件
/// 2. 前 12 字节为 AES-GCM Nonce，剩余部分为密文
/// 3. 使用硬编码的 AES-256 密钥解密，得到明文 Token
/// 4. 返回 trim 后的 Token 字符串
#[cfg(target_os = "windows")]
fn load_token() -> Result<String, String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let token_path = std::path::PathBuf::from(program_data).join("Auroweave").join("data").join("ipc_token.bin");
    if !token_path.exists() {
        return Err("Token 文件不存在".to_string());
    }
    let content = std::fs::read(token_path).map_err(|e| e.to_string())?;
    
    // 使用 AES-256-GCM 解密 Token
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };
    const TOKEN_KEY: &[u8; 32] = b"AuroweaveIPCSecretKey2026_Secure";
    
    // 校验文件长度：至少需要 12 字节 Nonce
    if content.len() < 12 {
        return Err("Token 文件已损坏".to_string());
    }
    
    // 前 12 字节为 Nonce，其余为密文
    let key: &aes_gcm::Key<Aes256Gcm> = TOKEN_KEY.into();
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&content[..12]);
    let ciphertext = &content[12..];
    
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Token 解密失败".to_string())?;
        
    let token_str = String::from_utf8(plaintext)
        .map_err(|_| "Token UTF-8 解析失败".to_string())?;
        
    Ok(token_str.trim().to_string())
}

// 非 Windows 平台桩代码
#[cfg(not(target_os = "windows"))]
pub async fn send_ipc_request(_action: &str, _config_content: Option<&str>) -> Result<IpcResponse, String> {
    Err("当前平台不支持系统服务 IPC 控制".to_string())
}
