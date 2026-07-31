import type { ApiResponse, Subscription } from "@/types";
import { SUBSCRIPTION_FETCH_TIMEOUT_MS } from "@/constants";
import { invokeWithTimeout } from "./client";

/** 导入订阅（URL 方式）- 传入较长的拉取超时时间 */
export async function importSubscription(
  name: string,
  url: string,
  autoGroup: boolean = true
): Promise<ApiResponse<Subscription>> {
  return invokeWithTimeout<ApiResponse<Subscription>>(
    "subscription_import",
    { name, url, autoGroup },
    SUBSCRIPTION_FETCH_TIMEOUT_MS
  );
}

/** 获取所有已保存订阅 */
export async function getSubscriptions(): Promise<ApiResponse<Subscription[]>> {
  return invokeWithTimeout<ApiResponse<Subscription[]>>("subscription_get_all");
}

/** 删除订阅 */
export async function deleteSubscription(
  id: string
): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("subscription_delete", { id });
}

/** 手动刷新订阅 - 传入较长的拉取超时时间 */
export async function refreshSubscription(
  id: string
): Promise<ApiResponse<Subscription>> {
  return invokeWithTimeout<ApiResponse<Subscription>>(
    "subscription_refresh",
    { id },
    SUBSCRIPTION_FETCH_TIMEOUT_MS
  );
}

/** 切换/激活订阅（切换当前使用的订阅配置） */
export async function activateSubscription(
  id: string
): Promise<ApiResponse<Subscription>> {
  return invokeWithTimeout<ApiResponse<Subscription>>(
    "subscription_activate",
    { id },
    SUBSCRIPTION_FETCH_TIMEOUT_MS
  );
}
