/// IPC 命令 — 日志管理（读取 / 清理）
/// 作者: TanXiang
///
/// 日志目录统一使用 crate::get_log_dir()（= get_data_root()/logs），
/// 与 lib.rs 中 tauri-plugin-log 的写入目录保持一致。
/// Windows 为 %ProgramData%\Auroweave\logs，macOS 为 ~/Library/Application Support/Auroweave/logs。
use crate::error::ApiResponse;
use std::fs;

/// 读取主程序日志（最近 N 行，默认 500 行）
#[tauri::command]
pub async fn log_read_app(lines: Option<usize>) -> ApiResponse<String> {
    let limit = lines.unwrap_or(500);
    let log_path = crate::get_log_dir().join("auroweave.log");
    if !log_path.exists() {
        return ApiResponse::ok(String::new());
    }
    match fs::read_to_string(&log_path) {
        Ok(content) => {
            // 返回最后 N 行
            let all_lines: Vec<&str> = content.lines().collect();
            let start = all_lines.len().saturating_sub(limit);
            ApiResponse::ok(all_lines[start..].join("\n"))
        }
        Err(e) => ApiResponse::err(format!("读取主程序日志失败: {}", e), 500),
    }
}

/// 读取系统服务日志（最近 N 行，默认 500 行）
#[tauri::command]
pub async fn log_read_service(lines: Option<usize>) -> ApiResponse<String> {
    let limit = lines.unwrap_or(500);
    let log_path = crate::get_log_dir().join("service.log");
    if !log_path.exists() {
        return ApiResponse::ok(String::new());
    }
    match fs::read_to_string(&log_path) {
        Ok(content) => {
            // 返回最后 N 行
            let all_lines: Vec<&str> = content.lines().collect();
            let start = all_lines.len().saturating_sub(limit);
            ApiResponse::ok(all_lines[start..].join("\n"))
        }
        Err(e) => ApiResponse::err(format!("读取服务日志失败: {}", e), 500),
    }
}


/// 清空所有日志文件（主程序、服务、内核）
#[tauri::command]
pub async fn log_clear_all() -> ApiResponse<()> {
    log::info!("[logging] 用户请求清空所有日志文件");
    let log_dir = crate::get_log_dir();
    let files = [
        log_dir.join("auroweave.log"),
        log_dir.join("service.log"),
    ];
    let mut errors: Vec<String> = Vec::new();
    for path in &files {
        if path.exists() {
            if let Err(e) = fs::write(path, "") {
                errors.push(format!("{:?}: {}", path.file_name().unwrap_or_default(), e));
            } else {
                log::info!("[logging] 已清空日志文件: {:?}", path.file_name().unwrap_or_default());
            }
        }
    }
    if errors.is_empty() {
        ApiResponse::ok(())
    } else {
        ApiResponse::err(format!("部分日志清空失败: {}", errors.join("; ")), 500)
    }
}
