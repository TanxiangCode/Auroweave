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

