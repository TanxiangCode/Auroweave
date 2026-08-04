/**
 * 节点搜索与排序 Hook
 * 作者: TanXiang
 *
 * 职责：对原始节点列表进行关键词搜索过滤与多维度排序
 */
import { ref, computed } from "vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import type { ProxyNode, NodeSortConfig, NodeSortKey } from "@/types";

/** 排序标签映射 */
const sortLabels: Record<string, string> = {
  default: "默认排序",
  name: "按名称",
  latency: "按延迟",
  protocol: "按协议",
};

/** 排序键循环顺序 */
const sortKeyCycle: Array<NodeSortKey> = ["default", "name", "latency", "protocol"];

/**
 * 节点搜索与排序 Hook
 *
 * @param rawNodes - 来自 useProxyGroups 的原始节点列表（ComputedRef）
 */
export function useNodeFilter(rawNodes: { value: ProxyNode[] }) {
  const speedtestStore = useSpeedtestStore();

  const searchText = ref("");
  const sortConfig = ref<NodeSortConfig>({ key: "default", order: "asc" });

  /** 经搜索过滤 + 排序后的节点列表 */
  const displayNodes = computed<ProxyNode[]>(() => {
    let nodes = rawNodes.value;

    // 搜索过滤
    if (searchText.value.trim()) {
      const kw = searchText.value.trim().toLowerCase();
      nodes = nodes.filter(
        (n) =>
          n.tag.toLowerCase().includes(kw) || n.type.toLowerCase().includes(kw)
      );
    }

    // 排序
    if (sortConfig.value.key === "default") return nodes;

    const sorted = [...nodes];
    const { key, order } = sortConfig.value;
    const multiplier = order === "asc" ? 1 : -1;

    sorted.sort((a, b) => {
      if (key === "name") {
        return a.tag.localeCompare(b.tag, "zh-CN") * multiplier;
      } else if (key === "protocol") {
        return a.type.localeCompare(b.type) * multiplier;
      } else if (key === "latency") {
        const aLat = speedtestStore.latencyMap[a.tag];
        const bLat = speedtestStore.latencyMap[b.tag];
        // 未测试的排最后
        if (aLat === undefined && bLat === undefined) return 0;
        if (aLat === undefined) return 1;
        if (bLat === undefined) return -1;
        return (aLat - bLat) * multiplier;
      }
      return 0;
    });

    return sorted;
  });

  /** 循环切换排序键 */
  function cycleSortKey() {
    const idx = sortKeyCycle.indexOf(sortConfig.value.key);
    const nextKey = sortKeyCycle[(idx + 1) % sortKeyCycle.length];
    sortConfig.value = { key: nextKey, order: "asc" };
  }

  /** 切换排序方向（升序/降序） */
  function toggleSortOrder() {
    sortConfig.value = {
      ...sortConfig.value,
      order: sortConfig.value.order === "asc" ? "desc" : "asc",
    };
  }

  /** 清空搜索文本 */
  function clearSearch() {
    searchText.value = "";
  }

  return {
    // 状态
    searchText,
    sortConfig,
    sortLabels,
    // 计算属性
    displayNodes,
    // 方法
    cycleSortKey,
    toggleSortOrder,
    clearSearch,
  };
}
