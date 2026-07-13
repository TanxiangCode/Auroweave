/// Windows 系统服务 IPC 具名管道客户端
/// 作者: TanXiang
use serde::{Deserialize, Serialize};

const PIPE_NAME: &str = r"\\.\pipe\Auroweave.Core.Control";

#[derive(Debug, Serialize)]
struct IpcRequest {
    action: String,
    token: String,
    config: Option<String>,
}

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

    // 读取受保护的安全 Token
    let token = match load_token() {
        Ok(t) => t,
        Err(e) => return Err(format!("本地服务验证 Token 获取失败: {}", e)),
    };

    // 连接服务管道
    let mut client = match tokio::net::windows::named_pipe::ClientOptions::new().open(PIPE_NAME) {
        Ok(c) => c,
        Err(e) => return Err(format!("无法连接至服务控制管道 (可能服务未运行): {}", e)),
    };

    let req = IpcRequest {
        action: action.to_string(),
        token,
        config: config_content.map(|s| s.to_string()),
    };

    let req_bytes = serde_json::to_vec(&req).map_err(|e| format!("序列化请求失败: {}", e))?;
    
    client.write_all(&req_bytes).await.map_err(|e| format!("向管道写入指令失败: {}", e))?;
    client.flush().await.map_err(|e| format!("清空管道缓冲区失败: {}", e))?;

    let mut buf = vec![0u8; 65536];
    let n = client.read(&mut buf).await.map_err(|e| format!("读取管道响应失败: {}", e))?;
    if n == 0 {
        return Err("服务未返回任何响应数据".to_string());
    }

    let resp: IpcResponse = serde_json::from_slice(&buf[..n])
        .map_err(|e| format!("解析管道返回数据失败: {}", e))?;

    Ok(resp)
}

#[cfg(target_os = "windows")]
fn load_token() -> Result<String, String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let token_path = std::path::PathBuf::from(program_data).join("Auroweave").join("token.txt");
    if !token_path.exists() {
        return Err("Token 文件不存在".to_string());
    }
    let content = std::fs::read_to_string(token_path).map_err(|e| e.to_string())?;
    Ok(content.trim().to_string())
}

// 非 Windows 平台桩代码
#[cfg(not(target_os = "windows"))]
pub async fn send_ipc_request(_action: &str, _config_content: Option<&str>) -> Result<IpcResponse, String> {
    Err("当前平台不支持系统服务 IPC 控制".to_string())
}
