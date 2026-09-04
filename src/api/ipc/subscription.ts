import type { ApiResponse, Subscription } from "@/types";
import { SUBSCRIPTION_FETCH_TIMEOUT_MS } from "@/constants";
import { invokeWithTimeout } from "./client";

/**
 * 导入订阅（URL 方式）- 传入较长的拉取超时时间
 *
 * 注意：Rust 端 `subscription_import` 的 `_auto_group` 参数当前仅保留占位、未实现，
 * 传入值不会影响导入行为（节点自动分组由后端默认策略处理）。前端调用保持参数透传。
 */
export async function importSubscription(
  name: string,
  url: string,
  /** 后端当前忽略此参数（Rust 侧 `_auto_group` 未实现） */
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
  /** 后端当前忽略此参数（Rust 侧 `_auto_group` 未实现） */
  autoGroup: boolean = true
): Promise<ApiResponse<Subscription>> {
  return invokeWithTimeout<ApiResponse<Subscription>>(
    "subscription_import_content",
    { name, content, sourceType, filePath, autoGroup },
    SUBSCRIPTION_FETCH_TIMEOUT_MS
  );
}

/**
 * 更新订阅元数据（部分更新语义）
 *
 * 仅提交需要修改的字段：undefined 字段会被 Tauri invoke 序列化时忽略
 * （JSON.stringify 与 invoke args 均跳过 undefined），后端已修复为
 * 逐字段合并、undefined 不再覆盖已有值。传全量对象反而可能用过期值覆盖。
 */
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

/** 查看订阅完整配置（清洗前原始文本、清洗后节点列表、最终运行时配置） */
export async function inspectSubscription(
  id: string
): Promise<ApiResponse<import("@/types").SubscriptionInspectData>> {
  return invokeWithTimeout<ApiResponse<import("@/types").SubscriptionInspectData>>(
    "subscription_inspect",
    { id }
  );
}

/** 本地规则集缓存状态（geosite-cn / geoip-cn .srs） */
export interface RuleSetStatus {
  geosite_exists: boolean;
  geosite_size: number;
  geosite_modified: number | null;
  geoip_exists: boolean;
  geoip_size: number;
  geoip_modified: number | null;
}

/** 查询本地规则集缓存状态 */
export async function getRuleSetStatus(): Promise<ApiResponse<RuleSetStatus>> {
  return invokeWithTimeout<ApiResponse<RuleSetStatus>>("ruleset_get_status");
}

/** 强制更新规则集（绕过缓存下载最新 .srs 并重建配置） */
export async function forceUpdateRuleSets(): Promise<
  ApiResponse<[boolean, boolean, number, number]>
> {
  return invokeWithTimeout<ApiResponse<[boolean, boolean, number, number]>>(
    "ruleset_force_update",
    {},
    30000
  );
}
