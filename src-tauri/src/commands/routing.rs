/// IPC 命令 — App-Matrix 应用分流路由
/// 作者: TanXiang
use crate::error::{ApiResponse, AppError};
use crate::system::process::{get_active_processes, SystemProcess};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;
use tracing::info;

/// 合法的自定义规则类型白名单（与 config_builder 生成路由规则时支持的类型一致）
const VALID_RULE_TYPES: [&str; 5] = [
    "domain",
    "domain_suffix",
    "domain_keyword",
    "domain_regex",
    "ip_cidr",
];

/// 从指定配置提取真实存在的 outbound tag，供保存校验与配置重建共用。
/// 不预置 direct/block/proxy：调用方必须以当前配置实际声明为准。
pub fn outbound_tags_from_config(config: &serde_json::Value) -> std::collections::HashSet<String> {
    config
        .get("outbounds")
        .and_then(|outbounds| outbounds.as_array())
        .map(|outbounds| {
            outbounds
                .iter()
                .filter_map(|outbound| outbound.get("tag").and_then(|tag| tag.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// 读取 config.json 提取所有出站 tag 集合，用于校验 outbound_tag 合法性
///
/// 校验策略：
/// 1. 基础出站 "direct" / "block" / "proxy" 始终合法
/// 2. 读取 config.json 的 outbounds[].tag 补充当前节点/策略组 tag
/// 3. config.json 缺失或解析失败时退化为仅基础白名单（宽松降级，不阻断操作）
fn known_outbound_tags() -> std::collections::HashSet<String> {
    let mut tags: std::collections::HashSet<String> = ["direct", "block", "proxy"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let config_path = crate::get_config_dir().join("config.json");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            tags.extend(outbound_tags_from_config(&val));
        }
    }
    tags
}

/// 校验 outbound_tag 在已知出站集合内，防止写入 sing-box 无法解析的规则导致内核启动失败
fn validate_outbound_tag(tag: &str) -> Result<(), AppError> {
    let trimmed = tag.trim();
    if trimmed.is_empty() || trimmed == "default" {
        // 空值/default 由调用方按"清除规则"语义处理
        return Ok(());
    }
    if known_outbound_tags().contains(trimmed) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "未知的出站标识: {}（必须为 direct/block/proxy 或当前配置中的节点名称）",
            trimmed
        )))
    }
}

/// 校验单条自定义规则：类型白名单 + 出站集合 + 正则可编译
fn validate_custom_rule(rule: &CustomRuleItem) -> Result<(), AppError> {
    if !VALID_RULE_TYPES.contains(&rule.rule_type.as_str()) {
        return Err(AppError::Validation(format!(
            "无效的规则类型: {}（允许: {}）",
            rule.rule_type,
            VALID_RULE_TYPES.join(" / ")
        )));
    }
    if rule.payload.trim().is_empty() {
        return Err(AppError::Validation("规则匹配内容不能为空".to_string()));
    }
    validate_outbound_tag(&rule.outbound_tag)?;
    // domain_regex 类型在保存前用 regex 编译校验，防止坏正则进入 config.json 导致内核启动失败
    if rule.rule_type == "domain_regex" {
        if let Err(e) = regex::Regex::new(rule.payload.trim()) {
            return Err(AppError::Validation(format!(
                "domain_regex 正则表达式无效: {}",
                e
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppRuleItem {
    pub process_name: String,
    pub outbound_tag: String,
}

/// 获取当前系统活跃应用进程列表
#[tauri::command]
pub async fn routing_get_processes() -> Result<ApiResponse<Vec<SystemProcess>>, AppError> {
    let list = get_active_processes();
    Ok(ApiResponse::ok(list))
}

/// 获取 app-rules.json 持久化路径
pub fn get_app_rules_path() -> std::path::PathBuf {
    crate::get_config_dir().join("app-rules.json")
}

/// 内部加载应用分流规则函数（供配置生成器与设置重构函数复用）
pub fn load_app_rules_internal() -> HashMap<String, String> {
    let rules_file = get_app_rules_path();
    if !rules_file.exists() {
        return HashMap::new();
    }
    let content = fs::read_to_string(&rules_file).unwrap_or_else(|_| "{}".to_string());
    serde_json::from_str(&content).unwrap_or_default()
}

/// 获取已保存的 App-Matrix 进程规则
#[tauri::command]
pub async fn routing_get_app_rules(
    _app_handle: AppHandle,
) -> Result<ApiResponse<HashMap<String, String>>, AppError> {
    Ok(ApiResponse::ok(load_app_rules_internal()))
}

/// 保存单个应用的进程分流规则
#[tauri::command]
pub async fn routing_save_app_rule(
    app_handle: AppHandle,
    process_name: String,
    outbound_tag: String,
) -> Result<ApiResponse<()>, AppError> {
    info!("设置应用 [{}] 绑定出站: {}", process_name, outbound_tag);
    // 进程名也做基础校验（去首尾空白，防空白名污染规则文件）
    let process_name = process_name.trim().to_string();
    if process_name.is_empty() {
        return Ok(ApiResponse::err(
            AppError::Validation("进程名不能为空".to_string()),
            400,
        ));
    }
    let outbound_tag = outbound_tag.trim().to_string();
    if let Err(e) = validate_outbound_tag(&outbound_tag) {
        return Ok(ApiResponse::err(e, 400));
    }
    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);
    let rules_file = get_app_rules_path();

    let mut map = load_app_rules_internal();

    if outbound_tag.is_empty() || outbound_tag == "default" {
        map.remove(&process_name);
    } else {
        map.insert(process_name, outbound_tag);
    }

    // 原子写入 app-rules.json（先写 tmp 再 rename，避免截断损坏）
    let json_str = serde_json::to_string_pretty(&map)
        .map_err(|e| AppError::Config(format!("序列化应用分流规则失败: {}", e)))?;
    crate::fs_utils::atomic_write(&rules_file, json_str.as_bytes())
        .map_err(|e| AppError::Config(format!("写入应用分流规则失败: {}", e)))?;

    // 重新根据最新 app-rules 重建 config.json（失败记录详情，不再静默丢弃）
    if let Err(e) = crate::commands::settings::rebuild_config_from_settings(&app_handle) {
        log::error!(
            "[routing] 重建 config.json 失败（应用分流规则可能未生效）: {}",
            e
        );
    }

    // 若内核正在运行，通过统一自愈流程让新 config.json 生效
    // （sing-box 无运行时配置热重载 API：PUT /configs 恒 204 空实现）
    let running_res = crate::commands::settings::core_query_running(app_handle.clone()).await;
    if running_res.data.unwrap_or(false) {
        if let Err(e) = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
            log::error!(
                "[routing] 规则已保存但内核重启失败（下次启动自动生效）: {}",
                e
            );
        }
        info!("应用分流规则已更新并同步至 sing-box");
    }

    Ok(ApiResponse::ok(()))
}

/// 自定义分流规则项模型
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomRuleItem {
    pub id: String,
    pub rule_type: String, // "domain" | "domain_suffix" | "domain_keyword" | "domain_regex" | "ip_cidr"
    pub payload: String,   // 匹配内容
    pub outbound_tag: String, // "proxy" | "direct" | "block" | 节点名称
    pub enabled: bool,
    pub description: Option<String>,
}

/// 获取 custom-rules.json 持久化路径
pub fn get_custom_rules_path() -> std::path::PathBuf {
    crate::get_config_dir().join("custom-rules.json")
}

/// 内部加载自定义分流规则函数（供配置生成器复用）
pub fn load_custom_rules_internal() -> Vec<CustomRuleItem> {
    let rules_file = get_custom_rules_path();
    if !rules_file.exists() {
        return Vec::new();
    }
    let content = fs::read_to_string(&rules_file).unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&content).unwrap_or_default()
}

/// 内部保存自定义规则并触发配置重建与热重载
async fn save_custom_rules_internal(
    app_handle: &AppHandle,
    rules: &[CustomRuleItem],
) -> Result<(), AppError> {
    // 保存前逐条校验：类型白名单 + 出站集合 + 正则可编译
    for rule in rules {
        if rule.enabled {
            validate_custom_rule(rule)?;
        }
    }

    let config_dir = crate::get_config_dir();
    let _ = fs::create_dir_all(&config_dir);
    let rules_file = get_custom_rules_path();

    let json_str = serde_json::to_string_pretty(rules)
        .map_err(|e| AppError::Config(format!("序列化自定义规则失败: {}", e)))?;
    crate::fs_utils::atomic_write(&rules_file, json_str.as_bytes())
        .map_err(|e| AppError::Config(format!("写入自定义规则失败: {}", e)))?;

    // 重新根据最新规则重建 config.json（失败记录详情，不再静默丢弃）
    if let Err(e) = crate::commands::settings::rebuild_config_from_settings(app_handle) {
        log::error!(
            "[routing] 重建 config.json 失败（自定义规则可能未生效）: {}",
            e
        );
    }

    // 若内核正在运行，通过统一自愈流程让新 config.json 生效
    // （sing-box 无运行时配置热重载 API：PUT /configs 恒 204 空实现）
    let running_res = crate::commands::settings::core_query_running(app_handle.clone()).await;
    if running_res.data.unwrap_or(false) {
        if let Err(e) = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await {
            log::error!(
                "[routing] 规则已保存但内核重启失败（下次启动自动生效）: {}",
                e
            );
        }
        info!("自定义分流规则已更新并同步至 sing-box");
    }

    Ok(())
}

/// 获取所有自定义分流规则
#[tauri::command]
pub async fn routing_get_custom_rules(
    _app_handle: AppHandle,
) -> Result<ApiResponse<Vec<CustomRuleItem>>, AppError> {
    Ok(ApiResponse::ok(load_custom_rules_internal()))
}

/// 批量保存/排序自定义分流规则
#[tauri::command]
pub async fn routing_save_custom_rules(
    app_handle: AppHandle,
    rules: Vec<CustomRuleItem>,
) -> Result<ApiResponse<()>, AppError> {
    save_custom_rules_internal(&app_handle, &rules).await?;
    Ok(ApiResponse::ok(()))
}

/// 添加或更新单条自定义规则
#[tauri::command]
pub async fn routing_add_custom_rule(
    app_handle: AppHandle,
    rule: CustomRuleItem,
) -> Result<ApiResponse<()>, AppError> {
    let mut rules = load_custom_rules_internal();
    if let Some(pos) = rules.iter().position(|r| r.id == rule.id) {
        rules[pos] = rule;
    } else {
        rules.push(rule);
    }
    save_custom_rules_internal(&app_handle, &rules).await?;
    Ok(ApiResponse::ok(()))
}

/// 删除单条自定义规则
#[tauri::command]
pub async fn routing_delete_custom_rule(
    app_handle: AppHandle,
    id: String,
) -> Result<ApiResponse<()>, AppError> {
    info!("[routing] 删除自定义分流规则: {}", id);
    let mut rules = load_custom_rules_internal();
    rules.retain(|r| r.id != id);
    save_custom_rules_internal(&app_handle, &rules).await?;
    Ok(ApiResponse::ok(()))
}

/// 分流规则导出文件格式（App-Matrix + 自定义规则全量备份）
#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingExport {
    pub version: u32,
    /// 导出时间（毫秒）
    pub exported_at: i64,
    /// App-Matrix 进程分流规则（进程名 -> 出站 tag）
    pub app_rules: HashMap<String, String>,
    /// 自定义分流规则
    pub custom_rules: Vec<CustomRuleItem>,
}

/// 导出全部分流规则为 JSON 字符串（前端拿去写文件/剪贴板）
#[tauri::command]
pub async fn routing_export_rules() -> Result<ApiResponse<String>, AppError> {
    let export = RoutingExport {
        version: 1,
        exported_at: chrono::Utc::now().timestamp_millis(),
        app_rules: load_app_rules_internal(),
        custom_rules: load_custom_rules_internal(),
    };
    match serde_json::to_string_pretty(&export) {
        Ok(json) => Ok(ApiResponse::ok(json)),
        Err(e) => Ok(ApiResponse::err(format!("序列化导出数据失败: {}", e), 500)),
    }
}

/// 导入分流规则 JSON（与现有规则按 id/进程名合并去重，不覆盖未提及的既有规则）
///
/// 导入校验失败的单条跳过并计入 skipped，全部失败返回错误；
/// merge=true 时合并（默认），merge=false 时整体替换现有规则。
#[tauri::command]
pub async fn routing_import_rules(
    app_handle: AppHandle,
    json_content: String,
    merge: Option<bool>,
) -> Result<ApiResponse<(usize, usize)>, AppError> {
    let merge = merge.unwrap_or(true);
    let data: RoutingExport = serde_json::from_str(json_content.trim())
        .map_err(|e| AppError::Validation(format!("导入文件格式错误: {}", e)))?;

    if data.version != 1 {
        return Ok(ApiResponse::err(
            format!("不支持的导出版本: {}", data.version),
            400,
        ));
    }

    // ---- 自定义规则：逐条校验后合并/替换 ----
    let mut custom_rules = if merge {
        load_custom_rules_internal()
    } else {
        Vec::new()
    };
    let mut imported_count = 0usize;
    let mut skipped = 0usize;
    for rule in data.custom_rules {
        // 校验失败的条目跳过（导入文件可能来自外部，不可信）
        if validate_custom_rule(&rule).is_err() {
            skipped += 1;
            continue;
        }
        // 合并语义：同 id 覆盖，否则追加
        if let Some(pos) = custom_rules.iter().position(|r| r.id == rule.id) {
            custom_rules[pos] = rule;
        } else {
            custom_rules.push(rule);
        }
        imported_count += 1;
    }

    // ---- App-Matrix：进程名去重合并（导入值覆盖同进程现有值）----
    let mut app_rules = if merge {
        load_app_rules_internal()
    } else {
        HashMap::new()
    };
    for (proc, outbound) in data.app_rules {
        if !proc.is_empty() && !outbound.is_empty() {
            app_rules.insert(proc, outbound);
            imported_count += 1;
        } else {
            skipped += 1;
        }
    }

    if imported_count == 0 {
        return Ok(ApiResponse::err(
            format!("导入完成但无有效规则（跳过 {} 条）", skipped),
            400,
        ));
    }

    // 持久化：App-Matrix 走原子写，自定义规则走统一保存（含校验+重建+热重载）
    if !app_rules.is_empty() {
        let app_rules_path = get_app_rules_path();
        let json_str = serde_json::to_string_pretty(&app_rules)
            .map_err(|e| AppError::Config(format!("序列化进程规则失败: {}", e)))?;
        crate::fs_utils::atomic_write(&app_rules_path, json_str.as_bytes())
            .map_err(|e| AppError::Config(format!("写入进程规则失败: {}", e)))?;
    }
    save_custom_rules_internal(&app_handle, &custom_rules).await?;

    info!(
        "[routing] 规则导入完成: 导入 {} 条, 跳过 {} 条",
        imported_count, skipped
    );
    Ok(ApiResponse::ok((imported_count, skipped)))
}
