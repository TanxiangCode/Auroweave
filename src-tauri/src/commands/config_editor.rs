/// 配置编辑器模块（plan-Q）：JSON Schema 导出 + config.json 安全编辑
/// 作者: TanXiang
///
/// 分层（设计见 plans/plan-Q-config-editor.md A.3/A.4）：
/// - L1 JSON 语法校验（serde_json 解析，行号列号直接可用）
/// - L3 语义校验（sing-box check -c 临时文件，"check 不过绝不写回"是硬边界）
/// - L2 schema 层校验已按 A.4-1 降级砍掉（check 覆盖结构性/引用性错误），
///   只保留 schema 文件导出能力供外部编辑器使用
///
/// 写回链：备份现有 config.json → 原子写新内容（不自动重启内核，
/// 由前端弹窗给「立即重启内核生效」确认按钮）。
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// 编辑器保存链的结果：被拦（syntax/check）或已写入（saved）
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EditSaveResult {
    /// L1 拦截：JSON 语法错误（行号列号从 1 起，供面板定位）
    SyntaxBlocked {
        line: usize,
        column: usize,
        message: String,
    },
    /// L3 拦截：sing-box check 未通过（location 为内核报错的定位段，可能为空）
    CheckBlocked {
        location: String,
        message: String,
        /// 解析不出定位时兜底展示的完整输出（不吞错）
        raw_output: String,
    },
    /// 全过并已安全写回（backup 已先行更新）
    Saved { size: usize },
}

/// 导出当前内核的 JSON Schema 文件（plan-Q Q1）
///
/// 调用本仓库 sing-box 的 `schema -o` 生成 Draft 2020-12 schema
/// （反映裁剪构建实际包含的功能，比官方在线 schema 更贴合），
/// 写入数据目录 config/schema.json 并返回绝对路径，供 VS Code 等外部编辑器使用。
#[tauri::command]
pub async fn config_export_schema() -> Result<ApiResponse<String>, AppError> {
    let binary = crate::core::sidecar::SidecarManager::resolve_binary_path()?;
    let schema_path = crate::get_config_dir().join("schema.json");

    let output = tokio::process::Command::new(&binary)
        .arg("schema")
        .arg("-o")
        .arg(&schema_path)
        .output()
        .await
        .map_err(|e| AppError::Sidecar(format!("调用 sing-box schema 失败: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Sidecar(format!(
            "sing-box schema 导出失败: {}",
            stderr.trim()
        )));
    }

    let size = std::fs::metadata(&schema_path)
        .map(|m| m.len())
        .unwrap_or(0);
    if size < 1024 {
        // 正常应为 444KB 级；异常小文件说明导出被截断或格式错误
        return Err(AppError::Sidecar(format!(
            "schema 文件异常（仅 {} 字节），疑似导出被截断",
            size
        )));
    }

    info!(
        "[config-editor] JSON Schema 已导出: {:?} ({} bytes)",
        schema_path, size
    );
    Ok(ApiResponse::ok(schema_path.to_string_lossy().to_string()))
}

/// 加载当前 config.json 原文供编辑器展示
#[tauri::command]
pub async fn config_editor_load() -> Result<ApiResponse<EditLoadResult>, AppError> {
    let config_path = crate::get_config_dir().join("config.json");
    if !config_path.exists() {
        return Err(AppError::Config(
            "config.json 尚未生成（请先导入订阅）".to_string(),
        ));
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| AppError::Io(format!("读取 config.json 失败: {}", e)))?;

    let size = content.len();
    if size > 1024 * 1024 {
        warn!("[config-editor] 加载的 config.json 超过 1MB（{} 字节），textarea 编辑体验可能受限", size);
    }

    Ok(ApiResponse::ok(EditLoadResult {
        content,
        size,
        path: config_path.to_string_lossy().to_string(),
    }))
}

/// 编辑器加载返回：原文 + 大小（前端展示提示）+ 文件路径
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EditLoadResult {
    pub content: String,
    pub size: usize,
    pub path: String,
}

/// 校验并安全写回 config.json（plan-Q Q2 主命令）
///
/// 顺序严格按设计 A.3：L1 语法 → L3 check → 备份 + 原子写。
/// 写回后不自动重启内核——由前端弹窗给确认按钮（用户可能还要连改几处）。
#[tauri::command]
pub async fn config_editor_save(
    content: String,
) -> Result<ApiResponse<EditSaveResult>, AppError> {
    let config_dir = crate::get_config_dir();
    let config_path = config_dir.join("config.json");

    // L1: JSON 语法校验（serde 行列号从 1 起，直接供面板定位）
    let _parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return Ok(ApiResponse::ok(EditSaveResult::SyntaxBlocked {
                line: e.line().max(1),
                column: e.column().max(1),
                message: e.to_string(),
            }));
        }
    };

    // L3: 写临时文件 → sing-box check → 删除临时文件
    let binary = crate::core::sidecar::SidecarManager::resolve_binary_path()?;
    let tmp_path = config_dir.join("config_edit_check.json");
    crate::fs_utils::atomic_write(&tmp_path, content.as_bytes())
        .map_err(|e| AppError::Io(format!("写入校验临时文件失败: {}", e)))?;

    let check_output = tokio::process::Command::new(&binary)
        .arg("check")
        .arg("-c")
        .arg(&tmp_path)
        // FATAL 带 ANSI 颜色码，禁用后解析才稳定
        .arg("--disable-color")
        .output()
        .await;
    let _ = std::fs::remove_file(&tmp_path);

    let output = check_output
        .map_err(|e| AppError::Sidecar(format!("调用 sing-box check 失败: {}", e)))?;
    if !output.status.success() {
        let raw = merge_output(&output.stdout, &output.stderr);
        let (location, message) = parse_fatal(&raw);
        return Ok(ApiResponse::ok(EditSaveResult::CheckBlocked {
            location,
            message,
            raw_output: raw,
        }));
    }

    // 安全写回：备份现有 → 原子写新内容（与订阅刷新链同一备份文件语义）
    if config_path.exists() {
        let backup_path = config_dir.join("config.backup.json");
        if let Err(e) = std::fs::copy(&config_path, &backup_path) {
            return Err(AppError::Io(format!("备份 config.json 失败: {}", e)));
        }
    }
    let size = content.len();
    crate::fs_utils::atomic_write(&config_path, content.as_bytes())
        .map_err(|e| AppError::Io(format!("写入 config.json 失败: {}", e)))?;

    info!("[config-editor] config.json 已编辑保存（{} 字节），改动前内容已入 config.backup.json", size);
    Ok(ApiResponse::ok(EditSaveResult::Saved { size }))
}

/// 重启内核使编辑后的 config.json 生效（plan-Q Q2）
///
/// 编辑器保存不自动重启（用户可能还要连改几处），此命令由弹窗的
/// 「立即重启内核生效」确认按钮调用。
///
/// 【严禁改走 apply_core_mode_with_fallback】那条链的步骤2会调
/// rebuild_config_from_settings 用设置态覆盖 inbounds/route/dns 段——
/// 用户刚手改的内容会被整体抹掉，编辑器价值链路断裂（二次复盘发现）。
/// 此处走纯重启链：不动 config.json 一个字节，仅按 run_mode 分流拉起
/// 进程（内核拒载错误恰恰在重启时刻暴露，属于编辑器正常反馈路径）。
#[tauri::command]
pub async fn config_editor_restart_core(
    app_handle: tauri::AppHandle,
) -> Result<ApiResponse<()>, AppError> {
    use tauri::Manager;

    let settings = crate::commands::settings::settings_get_internal(&app_handle);
    let config_path = crate::get_config_dir().join("config.json");
    let path_str = config_path.to_string_lossy().to_string();

    info!("[config-editor] 用户确认重启内核以应用编辑后的 config.json（纯重启，跳过 rebuild）");

    if settings.core.run_mode == "service" {
        // 服务模式：RELOAD_CONFIG 内嵌编辑后的配置文本下发（服务端写入自己的
        // config.json 再拉起内核）。IPC 协议约定 config 一律为内嵌文本——
        // 服务端拷贝有自己的 config.json，直接传路径会被当正文写入导致拒载。
        let config_content = std::fs::read_to_string(&config_path)
            .map_err(|e| AppError::Io(format!("读取 config.json 失败: {}", e)))?;
        let resp = crate::core::ipc_client::send_ipc_request("RELOAD_CONFIG", Some(&config_content))
            .await
            .map_err(|e| AppError::Sidecar(format!("与服务通信失败: {}", e)))?;
        if !resp.success {
            return Err(AppError::Sidecar(format!(
                "服务重启内核失败: {}",
                resp.error.unwrap_or(resp.status)
            )));
        }
        info!("[config-editor] 服务模式下内核已按编辑后配置重新拉起");
        // TUN 接管时无需系统代理
        if !settings.tun_enabled {
            if let Err(e) = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port) {
                log::error!("[config-editor] 重启内核后恢复系统代理失败: {}", e);
            }
        }
        return Ok(ApiResponse::ok(()));
    }

    // local 模式：stop → start 纯重启（与 SidecarManager::restart_privileged 同型）
    let sm = app_handle
        .state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>()
        .inner()
        .clone();
    sm.stop().await
        .map_err(|e| AppError::Sidecar(format!("停止内核失败: {}", e)))?;
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    sm.start(&path_str).await
        .map_err(|e| AppError::Sidecar(format!("重启内核失败（请检查编辑后的配置）: {}", e)))?;

    // 无 TUN 时恢复系统代理指向
    if !settings.tun_enabled {
        if let Err(e) = crate::system::sysproxy::set_system_proxy(true, settings.mixed_port) {
            log::error!("[config-editor] 重启内核后恢复系统代理失败: {}", e);
        }
    }
    Ok(ApiResponse::ok(()))
}

/// 合并 check 的 stdout/stderr 为完整输出（FATAL 日志走 stderr，个别信息走 stdout）
fn merge_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut parts = Vec::new();
    let out = String::from_utf8_lossy(stdout).trim().to_string();
    let err = String::from_utf8_lossy(stderr).trim().to_string();
    if !out.is_empty() {
        parts.push(out);
    }
    if !err.is_empty() {
        parts.push(err);
    }
    parts.join("\n")
}

/// 解析 check 的 FATAL 行为（定位, 原因）
///
/// 实测本仓库 sing-box 1.14.0 的四种 FATAL 形态（--disable-color 后）：
/// 1. `FATAL[0000] decode config at /path: dns.rules[0].rules: json: unknown field "rules"`
/// 2. `FATAL[0000] initialize router: parse rule-set[0]: open /nonexistent.srs: no such file or directory`
/// 3. `FATAL[0000] initialize dns router: dns rule[0]: rule-set not found: geosite-cn`
/// 4. `FATAL[0000] decode config at /path: invalid character 'b' ...: row 1, column 2`
///    （此类已被 L1 提前拦截，此函数仍兼容）
///
/// 解析规则：定位 = 冒号前最近的一个不含空格的段（形如 `dns.rules[0].rules`、
/// `rule-set[0]`、`dns rule[0]`）；原因 = 定位段之后的所有剩余文本。
/// 解析不出定位时返回 ("", 整行)，由调用方兜底展示 raw_output，绝不吞错。
fn parse_fatal(raw: &str) -> (String, String) {
    // 取最后一个 FATAL 行（多错时内核逐行 FATAL 后退出，末行为首因之后仍可能有更具体的）
    let fatal_line = raw
        .lines()
        .rev()
        .find(|l| l.contains("FATAL"))
        .unwrap_or_else(|| raw.lines().last().unwrap_or(""));
    if fatal_line.is_empty() {
        return (String::new(), raw.to_string());
    }

    // 剥掉 "FATAL[0000] " 前缀，剩余按 "原因前缀: " 拆段
    let body = strip_fatal_prefix(fatal_line);
    let segments: Vec<&str> = body.split(": ").collect();
    if segments.len() < 2 {
        return (String::new(), body.to_string());
    }

    // 解析规则：内核把定位段放在原因之前，从 segments[1] 起找首个
    // 符合定位形态的段。定位形态（实测四类 FATAL 归纳）：
    //   dots 形态：无空格且含 '.'（dns.rules[0].rules / log.level）
    //   序号形态：以 ']' 结尾（rule-set[0] / dns rule[0] / parse rule-set[0]）
    // 文件路径段（以 '/' 开头）跳过；原因文本（含空格且不以 ']' 结尾）天然不匹配。
    // 例：["decode config at /tmp/x.json", "dns.rules[0].rules", "json", "unknown field"]
    //      → 定位 = dns.rules[0].rules，原因 = "json: unknown field"
    // 例：["initialize dns router", "dns rule[0]", "rule-set not found", "geosite-cn"]
    //      → 定位 = dns rule[0]，原因 = "rule-set not found: geosite-cn"
    let mut location_idx = None;
    for i in 1..segments.len() {
        let seg = segments[i].trim();
        if seg.is_empty() || seg.starts_with('/') || seg.contains('\\') {
            continue; // 文件路径段不是定位
        }
        let is_dots = !seg.contains(' ') && seg.contains('.');
        let is_indexed = seg.ends_with(']');
        if (is_dots || is_indexed) && i + 1 < segments.len() {
            location_idx = Some(i);
            break;
        }
    }

    let location_idx = match location_idx {
        Some(i) => i,
        // 没有任何无空格段（不符合已知形态），整行原样返回
        None => return (String::new(), body.to_string()),
    };

    let location = segments[location_idx].trim().to_string();
    let message = if location_idx + 1 < segments.len() {
        segments[location_idx + 1..].join(": ")
    } else {
        String::new()
    };
    (location, message)
}

/// 剥掉 FATAL 日志行前缀（"FATAL[0000] " 或颜色码变体）
fn strip_fatal_prefix(line: &str) -> &str {
    let mut s = line.trim();
    // 兜底剥残留 ANSI 码（即使 --disable-color 失效也能解析）：
    // 转义序列形如 "\x1b[31m"，取行内首个 ESC 前的纯文本
    if let Some(start) = s.find('\u{1b}') {
        s = &s[..start];
    }
    // FATAL[0000] 或 FATAL: 前缀
    if let Some(rest) = s.strip_prefix("FATAL") {
        s = rest;
        // [0000] 时间戳
        if s.starts_with('[') {
            if let Some(end) = s.find(']') {
                s = &s[end + 1..];
            }
        }
        s = s.strip_prefix(':').unwrap_or(s);
        return s.trim_start();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_decode_config_unknown_field() {
        let raw = "FATAL[0000] decode config at /tmp/x.json: dns.rules[0].rules: json: unknown field \"rules\"";
        let (loc, msg) = parse_fatal(raw);
        assert_eq!(loc, "dns.rules[0].rules");
        assert_eq!(msg, "json: unknown field \"rules\"");
    }

    #[test]
    fn parse_initialize_router_rule_set() {
        let raw = "FATAL[0000] initialize router: parse rule-set[0]: open /nonexistent.srs: no such file or directory";
        let (loc, msg) = parse_fatal(raw);
        assert_eq!(loc, "parse rule-set[0]");
        assert_eq!(msg, "open /nonexistent.srs: no such file or directory");
    }

    #[test]
    fn parse_initialize_dns_rule_set_not_found() {
        let raw = "FATAL[0000] initialize dns router: dns rule[0]: rule-set not found: geosite-cn";
        let (loc, msg) = parse_fatal(raw);
        assert_eq!(loc, "dns rule[0]");
        assert_eq!(msg, "rule-set not found: geosite-cn");
    }

    #[test]
    fn parse_unlocatable_falls_back_to_full_line() {
        let raw = "FATAL[0000] some unexpected error without location segments";
        let (loc, msg) = parse_fatal(raw);
        assert_eq!(loc, "");
        assert!(!msg.is_empty(), "兜底必须整行返回，不吞错");
    }

    #[test]
    fn parse_bad_json_row_column() {
        let raw = "FATAL[0000] decode config at /tmp/bad1.json: invalid character 'b' looking for beginning of object key string: row 1, column 2";
        let (loc, msg) = parse_fatal(raw);
        // "row 1, column 2" 含空格不构成定位段，但 "column 2" 前的冒号拆分
        // 会留下可定位的最末无空格段；此用例在产品链路已被 L1 提前拦截，
        // 此处仅验证不 panic 且返回非空原因
        assert!(!msg.is_empty());
        assert!(loc.is_empty() || msg.contains("row"));
    }

    #[test]
    fn parse_takes_last_fatal_line() {
        let raw = "FATAL[0000] first error\nFATAL[0000] second error: with location\nsome trailing";
        let (loc, msg) = parse_fatal(raw);
        assert!(msg.contains("second error") || loc.contains("location"));
    }

    #[test]
    fn strip_prefix_handles_variants() {
        assert_eq!(strip_fatal_prefix("FATAL[0000] decode config: x"), "decode config: x");
        assert_eq!(strip_fatal_prefix("  FATAL[0000]  x"), "x");
    }

    #[test]
    fn merge_output_prefers_stderr_tail() {
        let raw = merge_output(b"info line", b"FATAL[0000] boom");
        assert!(raw.contains("info line"));
        assert!(raw.contains("FATAL[0000] boom"));
    }
}
