/**
 * IPC 客户端 API — App-Matrix 应用路由
 * 作者: TanXiang
 */
import type { ApiResponse } from "@/types";
import { invokeWithTimeout } from "./client";

export interface SystemProcessItem {
  pid: number;
  name: string;
  exe_path: string;
  icon_base64?: string;
}


/** 获取当前系统活跃应用进程列表 */
export async function getSystemProcesses(): Promise<ApiResponse<SystemProcessItem[]>> {
  return await invokeWithTimeout("routing_get_processes");
}

/** 获取已保存的 App-Matrix 进程分流规则 */
export async function getAppRules(): Promise<ApiResponse<Record<string, string>>> {
  return await invokeWithTimeout("routing_get_app_rules");
}

/** 保存单个进程的出站规则绑定 */
export async function saveAppRule(
  processName: string,
  outboundTag: string
): Promise<ApiResponse<void>> {
  return await invokeWithTimeout("routing_save_app_rule", { processName, outboundTag });
}

export interface CustomRuleItem {
  id: string;
  rule_type: "domain" | "domain_suffix" | "domain_keyword" | "domain_regex" | "ip_cidr";
  payload: string;
  outbound_tag: string;
  enabled: boolean;
  description?: string;
}

/** 获取所有自定义分流规则 */
export async function getCustomRules(): Promise<ApiResponse<CustomRuleItem[]>> {
  return await invokeWithTimeout("routing_get_custom_rules");
}

/** 批量保存/排序自定义分流规则 */
export async function saveCustomRules(rules: CustomRuleItem[]): Promise<ApiResponse<void>> {
  return await invokeWithTimeout("routing_save_custom_rules", { rules });
}

/** 添加或更新单条自定义规则 */
export async function addCustomRule(rule: CustomRuleItem): Promise<ApiResponse<void>> {
  return await invokeWithTimeout("routing_add_custom_rule", { rule });
}

/** 删除单条自定义规则 */
export async function deleteCustomRule(id: string): Promise<ApiResponse<void>> {
  return await invokeWithTimeout("routing_delete_custom_rule", { id });
}


/** 导出全部分流规则（App-Matrix + 自定义规则）为 JSON 字符串 */
export async function exportRoutingRules(): Promise<ApiResponse<string>> {
  return await invokeWithTimeout("routing_export_rules");
}

/** 导入分流规则 JSON（merge=true 按合并去重 / false 整体替换），
 *  返回 [导入条数, 跳过条数] */
export async function importRoutingRules(
  jsonContent: string,
  merge: boolean = true
): Promise<ApiResponse<[number, number]>> {
  return await invokeWithTimeout("routing_import_rules", {
    jsonContent,
    merge,
  });
}

/** 查询节点测速历史（SQLite 持久化） */
export async function getSpeedtestHistory(
  nodeTag: string,
  limit: number = 20
): Promise<
  ApiResponse<
    Array<{
      node_tag: string;
      download_bps: number;
      upload_bps: number;
      delay_ms?: number;
      tested_at: number;
    }>
  >
> {
  return await invokeWithTimeout("speedtest_get_history", { nodeTag, limit });
}
