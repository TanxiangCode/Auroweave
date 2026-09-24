import type { ProxyGroup, ProxyNode } from "@/types";

/** ClashAPI 策略组类型：这些类型本身不是可测速的物理节点。 */
const NON_TESTABLE_PROXY_TYPES = new Set(["selector", "urltest", "fallback", "loadbalance"]);

/** 判断 URL query 或当前选择指向的分组是否真实可见（包含虚拟 custom: 分组）。 */
export function hasProxyGroupTag(
  groups: ProxyGroup[],
  customGroups: ProxyGroup[],
  tag: string | undefined
): boolean {
  if (!tag) return false;
  return groups.some((group) => group.tag === tag)
    || customGroups.some((group) => group.tag === tag);
}

/** 从当前视图原始节点中提取可发起延迟测试的物理节点 tag。 */
export function getLatencyTestNodeTags(nodes: ProxyNode[]): string[] {
  return nodes
    .filter((node) => !NON_TESTABLE_PROXY_TYPES.has(node.type.toLowerCase()))
    .map((node) => node.tag);
}
