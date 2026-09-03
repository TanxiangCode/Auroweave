/**
 * IPC 封装层 — 代理节点相关命令
 * 作者: TanXiang
 */
import type { ApiResponse, ProxyGroup, ProxyNode } from "@/types";
import { invokeWithTimeout } from "./client";

/** 获取所有代理分组 */
export async function getProxyGroups(): Promise<ApiResponse<ProxyGroup[]>> {
  return invokeWithTimeout<ApiResponse<ProxyGroup[]>>("proxy_get_groups");
}

/** 获取分组内所有节点 */
export async function getGroupNodes(
  groupTag: string
): Promise<ApiResponse<ProxyNode[]>> {
  return invokeWithTimeout<ApiResponse<ProxyNode[]>>("proxy_get_group_nodes", {
    groupTag,
  });
}

/** 切换分组当前节点 */
export async function selectGroupNode(
  groupTag: string,
  nodeTag: string
): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("proxy_select_node", { groupTag, nodeTag });
}

/** 获取当前代理模式 */
export async function getProxyMode(): Promise<
  ApiResponse<"global" | "rule" | "direct">
> {
  return invokeWithTimeout<ApiResponse<"global" | "rule" | "direct">>("proxy_get_mode");
}

/** 切换代理模式 */
export async function setProxyMode(
  mode: "global" | "rule" | "direct"
): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("proxy_set_mode", { mode });
}

/** 关闭指定 ID 的活跃连接 */
export async function closeConnection(id: string): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("proxy_close_connection", { id });
}

/** 关闭全部活跃连接 */
export async function closeAllConnections(): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("proxy_close_all_connections");
}

/** 获取当前使用的 sing-box 版本号 */
export async function getSingboxVersion(): Promise<ApiResponse<string>> {
  return invokeWithTimeout<ApiResponse<string>>("proxy_get_singbox_version");
}

// === 分组配置持久化（P0 修复新增）===

/** 分组测速配置项 */
export interface GroupConfigPayload {
  interval?: number;
  tolerance?: number;
  url?: string;
}

/** 更新分组测速配置（interval / tolerance / url），持久化并在下次配置重建时生效 */
export async function updateGroupConfig(
  groupTag: string,
  config: GroupConfigPayload
): Promise<ApiResponse<boolean>> {
  return invokeWithTimeout<ApiResponse<boolean>>("group_update_config", {
    groupTag,
    config: JSON.stringify(config),
  });
}


