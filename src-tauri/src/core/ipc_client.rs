/// Windows 系统服务 IPC 具名管道客户端
/// 作者: TanXiang
///
/// 职责：主程序（Tauri 前端进程）通过具名管道向 AuroDaemon 系统服务发送控制指令。
///
/// IPC 通信协议（与服务端 crates/auroweave-svc/src/ipc.rs 保持一致）：
/// 1. **加载安全令牌**：从 `%ProgramData%\Auroweave\data\ipc_token.bin` 读取明文 Token
///    - Token 为安装时生成的随机 UUID v4，以明文存储，依赖文件 DACL（仅 SYSTEM/Admins 可读）保护
///    - 历史版本曾以硬编码 AES 密钥"加密"，因密钥公开而形同明文，已废弃
/// 2. **连接管道**：通过 `\\.\pipe\Auroweave.Core.Control` 连接服务端
///    - 若连接失败说明服务未运行，返回错误提示
/// 3. **发送请求（长度前缀分帧）**：8 字节 little-endian u64 长度头 + JSON 体，
///    循环 write 直至全部写出，支持大体积 config.json 传输
/// 4. **接收响应**：服务端以裸 JSON 直写管道，客户端循环 read 直至 JSON 解析成功或 EOF
///
/// 支持的 action：
/// - `GET_STATUS`：查询 sing-box 内核运行状态和 PID
/// - `RELOAD_CONFIG`：重载配置并重启内核（config 参数一律为内嵌配置文本）
/// - `SHUTDOWN_CORE`：停止内核进程
use serde::{Deserialize, Serialize};

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const PIPE_NAME: &str = r"\\.\pipe\Auroweave.Core.Control";

/// 分帧协议单帧上限，与服务端 MAX_FRAME_LEN 一致
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const MAX_FRAME_LEN: u64 = 16 * 1024 * 1024;

/// IPC 请求结构体
/// - `action`: 操作指令 (GET_STATUS / RELOAD_CONFIG / SHUTDOWN_CORE)
/// - `token`: 安全令牌（明文，依赖文件 ACL 保护）
/// - `config`: 可选的配置内容（仅 RELOAD_CONFIG 使用，一律为内嵌文本，不再接受路径）
#[derive(Debug, Serialize)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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
    // 从明文 token 文件读取（文件 DACL 仅允许 SYSTEM/Admins 与安装用户读取）
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

    // ---- 步骤3: 构建请求并按分帧协议发送 ----
    // 8 字节 LE u64 长度头 + JSON 体（与服务端 read_framed_request 对齐）
    let req = IpcRequest {
        action: action.to_string(),
        token,
        config: config_content.map(|s| s.to_string()),
    };
    let req_bytes = serde_json::to_vec(&req).map_err(|e| format!("序列化请求失败: {}", e))?;
    if req_bytes.len() as u64 > MAX_FRAME_LEN {
        return Err(format!("请求体过大 ({} bytes)，超出分帧协议上限", req_bytes.len()));
    }

    let frame = [req_bytes.len() as u64].concat(req_bytes.as_slice());
    client.write_all(&frame).await.map_err(|e| format!("向管道写入指令失败: {}", e))?;
    client.flush().await.map_err(|e| format!("清空管道缓冲区失败: {}", e))?;

    // ---- 步骤4: 接收并解析响应 ----
    // 服务端以裸 JSON 直写管道（无长度前缀）。管道是字节流，单次 read 不保证读满，
    // 循环 read 并增量尝试解析，直到 JSON 完整或连接 EOF。
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 8192];
    let resp: IpcResponse = loop {
        match client.read(&mut chunk).await {
            Ok(0) => {
                // EOF：用已收到的数据做最后一次解析尝试
                break serde_json::from_slice(&buf).map_err(|e| format!("服务响应不完整或已断开: {}", e))?;
            }
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() as u64 > MAX_FRAME_LEN {
                    return Err("服务响应超出大小上限".to_string());
                }
                // JSON 对象在完整前解析必然失败；完整后立即成功（serde_json::from_slice 全量校验）
                if let Ok(r) = serde_json::from_slice::<IpcResponse>(&buf) {
                    break r;
                }
                // 仍未解析成功：继续读取（服务端写完后保持连接直到客户端关闭）
            }
            Err(e) => return Err(format!("读取管道响应失败: {}", e)),
        }
    };

    log::info!("[ipc_client] 收到 IPC 管道反馈: success={}, status={}", resp.success, resp.status);
    Ok(resp)
}

/// 加载 IPC 安全令牌（明文）
///
/// 流程：
/// 1. 读取 `%ProgramData%\Auroweave\data\ipc_token.bin`
/// 2. 内容即安装时生成的随机 UUID v4 明文 Token
/// 3. 返回 trim 后的 Token 字符串
///
/// 安全说明：Token 文件的 DACL 仅授予 SYSTEM/Administrators 读取权限，
/// 依赖操作系统的访问控制而非混淆式加密。
#[cfg(target_os = "windows")]
fn load_token() -> Result<String, String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let token_path = std::path::PathBuf::from(program_data).join("Auroweave").join("data").join("ipc_token.bin");
    if !token_path.exists() {
        return Err("Token 文件不存在".to_string());
    }
    let content = std::fs::read(&token_path).map_err(|e| format!("读取 Token 文件失败: {}", e))?;
    let token_str = String::from_utf8(content).map_err(|_| "Token UTF-8 解析失败".to_string())?;
    if token_str.trim().is_empty() {
        return Err("Token 文件为空".to_string());
    }
    Ok(token_str.trim().to_string())
}

// 非 Windows 平台桩代码
#[cfg(not(target_os = "windows"))]
pub async fn send_ipc_request(_action: &str, _config_content: Option<&str>) -> Result<IpcResponse, String> {
    Err("当前平台不支持系统服务 IPC 控制".to_string())
}
