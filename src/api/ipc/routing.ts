/**
 * IPC 客户端 API — App-Matrix 应用路由
 * 作者: TanXiang
 */
import { invoke } from "@tauri-apps/api/core";
import type { ApiResponse } from "@/types";

export interface SystemProcessItem {
  pid: number;
  name: string;
  exe_path: string;
}

/** 获取当前系统活跃应用进程列表 */
export async function getSystemProcesses(): Promise<ApiResponse<SystemProcessItem[]>> {
  return await invoke("routing_get_processes");
}

/** 获取已保存的 App-Matrix 进程分流规则 */
export async function getAppRules(): Promise<ApiResponse<Record<string, string>>> {
  return await invoke("routing_get_app_rules");
}

/** 保存单个进程的出站规则绑定 */
export async function saveAppRule(
  processName: string,
  outboundTag: string
): Promise<ApiResponse<void>> {
  return await invoke("routing_save_app_rule", { processName, outboundTag });
}
