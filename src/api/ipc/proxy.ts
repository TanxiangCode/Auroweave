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
