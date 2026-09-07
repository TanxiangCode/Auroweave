/**
 * 节点搜索与排序 Hook
 * 作者: TanXiang
 *
 * 职责：对原始节点列表进行关键词搜索过滤、解锁状态筛选与多维度排序
 */
import { ref, computed } from "vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useUnlockStore, unlockSortWeight } from "@/stores/unlock.store";
import { useSettingsStore } from "@/stores/settings.store";
import type { ProxyNode, NodeSortConfig, NodeSortKey, UnlockServiceId, UnlockStatus } from "@/types";

/** 排序标签映射 */
const sortLabels: Record<string, string> = {
  default: "默认排序",
  name: "按名称",
  latency: "按延迟",
  protocol: "按协议",
  unlock: "按解锁",
};

/** 排序键循环顺序 */
const sortKeyCycle: Array<NodeSortKey> = ["default", "name", "latency", "protocol", "unlock"];

/**
 * 节点搜索与排序 Hook
 *
 * @param rawNodes - 来自 useProxyGroups 的原始节点列表（ComputedRef）
 */
export function useNodeFilter(rawNodes: { value: ProxyNode[] }) {
  const speedtestStore = useSpeedtestStore();
  const unlockStore = useUnlockStore();
  const settingsStore = useSettingsStore();

  const searchText = ref("");
  const sortConfig = ref<NodeSortConfig>({ key: "default", order: "asc" });

  /** 服务筛选状态（"gemini:yes" 形态；空串=未筛选） */
  const unlockFilter = ref("");

  /** 收藏置顶集合（来自 settings.pinned_nodes，Set 加速查重） */
  const pinnedSet = computed<Set<string>>(() => new Set(settingsStore.settings.pinned_nodes || []));

  /** 切换节点置顶收藏（持久化到设置） */
  async function togglePinned(nodeTag: string) {
    const current = new Set(settingsStore.settings.pinned_nodes || []);
    if (current.has(nodeTag)) {
      current.delete(nodeTag);
    } else {
      current.add(nodeTag);
    }
    await settingsStore.updateSettings({ pinned_nodes: Array.from(current) });
  }

  /** 经搜索 + 解锁筛选 + 置顶 + 排序后的节点列表 */
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

    // 解锁服务筛选（"service:status" 形态；无结果的节点不显示）
    if (unlockFilter.value) {
      const [svc, st] = unlockFilter.value.split(":") as [
        UnlockServiceId,
        UnlockStatus,
      ];
      nodes = nodes.filter(
        (n) => unlockStore.unlockMap[n.tag]?.services?.[svc] === st
      );
    }

    // 置顶分区：收藏节点恒排最前（优先于任何排序键，用户显式意图 > 自动排序）
    const pinned = nodes.filter((n) => pinnedSet.value.has(n.tag));
    const normal = nodes.filter((n) => !pinnedSet.value.has(n.tag));

    // 排序
    if (sortConfig.value.key === "default") return [...pinned, ...normal];

    const { key, order } = sortConfig.value;
    const multiplier = order === "asc" ? 1 : -1;

    const sortFn = (a: ProxyNode, b: ProxyNode): number => {
      if (key === "name") {
        return a.tag.localeCompare(b.tag, "zh-CN") * multiplier;
      } else if (key === "protocol") {
        return a.type.localeCompare(b.type) * multiplier;
      } else if (key === "latency") {
        const aLat = speedtestStore.latencyMap[a.tag];
        const bLat = speedtestStore.latencyMap[b.tag];

        // 延迟排序权重：
        // 有效延迟 (> 0): 保持原数值 (如 50, 120, 300)
        // 超时 / 失败 (<= 0): 赋予高权重 999990 排在有效节点之后
        // 未测试 (undefined): 赋予最高权重 999999 排在最后
        const getWeight = (lat?: number) => {
          if (lat === undefined) return 999999;
          if (lat <= 0) return 999990;
          return lat;
        };

        const wA = getWeight(aLat);
        const wB = getWeight(bLat);

        if (wA !== wB) {
          return (wA - wB) * multiplier;
        }
        return a.tag.localeCompare(b.tag, "zh-CN");
      } else if (key === "unlock") {
        // 解锁排序：档位（yes 最优）→ 检测时间新者优先 → 名称稳定序
        const wA = unlockSortWeight(unlockStore.unlockMap[a.tag]?.services);
        const wB = unlockSortWeight(unlockStore.unlockMap[b.tag]?.services);
        if (wA !== wB) {
          return (wA - wB) * multiplier;
        }
        const tsA = unlockStore.unlockMap[a.tag]?.tested_at ?? 0;
        const tsB = unlockStore.unlockMap[b.tag]?.tested_at ?? 0;
        if (tsA !== tsB) {
          return (tsB - tsA) * multiplier; // 时间降序 = 最新在前（乘 multiplier 支持反向）
        }
        return a.tag.localeCompare(b.tag, "zh-CN");
      }
      return 0;
    };

    // 置顶区与普通区各自排序后拼接（置顶恒在前，区内保持所选排序语义）
    return [...pinned].sort(sortFn).concat([...normal].sort(sortFn));
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
    unlockFilter,
    // 计算属性
    displayNodes,
    pinnedSet,
    // 方法
    cycleSortKey,
    toggleSortOrder,
    clearSearch,
    togglePinned,
  };
}
