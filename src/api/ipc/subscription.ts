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

/** 批量删除所有订阅 */
export async function deleteAllSubscriptions(): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("subscription_delete_all", {});
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

/** 直接导入内容（剪贴板节点列表/本地文件） */
export async function importContentSubscription(
  name: string,
  content: string,
  sourceType: string,
  filePath?: string,
  autoGroup: boolean = true
): Promise<ApiResponse<Subscription>> {
  return invokeWithTimeout<ApiResponse<Subscription>>(
    "subscription_import_content",
    { name, content, sourceType, filePath, autoGroup },
    SUBSCRIPTION_FETCH_TIMEOUT_MS
  );
}

/** 更新订阅元数据 */
export async function updateSubscriptionMeta(
  id: string,
  meta: {
    name?: string;
    url?: string;
    userAgent?: string;
    autoUpdateIntervalHours?: number;
    filterRule?: import("@/types").SubscriptionFilterRule;
  }
): Promise<ApiResponse<Subscription>> {
  return invokeWithTimeout<ApiResponse<Subscription>>(
    "subscription_update_meta",
    {
      id,
      name: meta.name,
      url: meta.url,
      userAgent: meta.userAgent,
      autoUpdateIntervalHours: meta.autoUpdateIntervalHours,
      filterRule: meta.filterRule,
    }
  );
}


