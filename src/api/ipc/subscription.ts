/**
 * IPC 封装层 — 订阅管理命令
 * 作者: TanXiang
 */
import { invoke } from "@tauri-apps/api/core";
import type { ApiResponse, Subscription } from "@/types";

/** 导入订阅（URL 方式） */
export async function importSubscription(
  name: string,
  url: string,
  autoGroup: boolean = true
): Promise<ApiResponse<Subscription>> {
  return invoke<ApiResponse<Subscription>>("subscription_import", {
    name,
    url,
    autoGroup,
  });
}

/** 获取所有已保存订阅 */
export async function getSubscriptions(): Promise<ApiResponse<Subscription[]>> {
  return invoke<ApiResponse<Subscription[]>>("subscription_get_all");
}

/** 删除订阅 */
export async function deleteSubscription(
  id: string
): Promise<ApiResponse<void>> {
  return invoke<ApiResponse<void>>("subscription_delete", { id });
}

/** 手动刷新订阅 */
export async function refreshSubscription(
  id: string
): Promise<ApiResponse<Subscription>> {
  return invoke<ApiResponse<Subscription>>("subscription_refresh", { id });
}
