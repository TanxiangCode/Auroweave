/**
 * Pinia Store — 代理节点与分组
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { ProxyGroup, ProxyNode } from "@/types";
import { getProxyGroups, getGroupNodes, selectGroupNode, setProxyMode } from "@/api/ipc/proxy";
import { useToast } from "@/composables/useToast";

export const useProxyStore = defineStore("proxy", () => {
  const toast = useToast();
  // ---- 状态 ----
  const groups = ref<ProxyGroup[]>([]);
  /** 各分组的节点缓存 key=groupTag */
  const nodeMap = ref<Map<string, ProxyNode[]>>(new Map());
  const proxyMode = ref<"global" | "rule" | "direct">("rule");
  const loading = ref(false);
  const error = ref<string | null>(null);

  // 从 localStorage 获取使用计数，格式为 Record<string, number>
  const groupUsage = ref<Record<string, number>>(
    JSON.parse(localStorage.getItem("auroweave_group_usage") || "{}")
  );

  // 常用分组列表通过计算属性得出：按使用次数从大到小排序，且只包含使用次数 > 1 的分组，限制长度为 4
  const recentGroups = computed(() => {
    return Object.entries(groupUsage.value)
      .filter(([_, count]) => count > 1)
      .sort((a, b) => b[1] - a[1])
      .map(([tag]) => tag)
      .slice(0, 4);
  });

  // 增加使用次数的方法
  function recordGroupUsage(groupTag: string) {
    const current = groupUsage.value[groupTag] || 0;
    groupUsage.value[groupTag] = current + 1;
    localStorage.setItem("auroweave_group_usage", JSON.stringify(groupUsage.value));
  }

  // ---- 计算属性 ----
  const activeGroup = computed(() =>
    groups.value.find((g) => g.type === "selector")
  );

  // ---- 动作 ----
  async function fetchGroups() {
    loading.value = true;
    error.value = null;
    const res = await getProxyGroups();
    if (res.success && res.data) {
      groups.value = res.data;
      // 默认将初始活跃分组载入最近列表作为兜底展示
      const primary = res.data.find((g) => g.type === "selector");
      if (primary && Object.keys(groupUsage.value).length === 0) {
        groupUsage.value[primary.tag] = 2; // 兜底：主策略组初始有 2 次，算作常用
        localStorage.setItem("auroweave_group_usage", JSON.stringify(groupUsage.value));
      }
    } else {
      error.value = res.error ?? "获取分组失败";
    }
    loading.value = false;
  }

  async function fetchGroupNodes(groupTag: string) {
    const res = await getGroupNodes(groupTag);
    if (res.success && res.data) {
      nodeMap.value.set(groupTag, res.data);
    }
  }

  async function selectNode(groupTag: string, nodeTag: string) {
    const res = await selectGroupNode(groupTag, nodeTag);
    if (res.success) {
      // 乐观更新 groups 中的 now 字段（左侧分组列表的"当前节点"文字）
      const group = groups.value.find((g) => g.tag === groupTag);
      if (group) group.now = nodeTag;

      // 修复 Vue 3 Map 响应式缺陷：
      // 直接修改 Map 内数组元素的属性不会被 Vue 追踪到，
      // 必须用新数组替换，才能触发依赖此 Map 的组件重新渲染。
      const nodes = nodeMap.value.get(groupTag);
      if (nodes) {
        const updated = nodes.map((n) => ({
          ...n,
          is_active: n.tag === nodeTag,
        }));
        nodeMap.value.set(groupTag, updated);
      }

      // 记录使用频次
      recordGroupUsage(groupTag);

      // 异步从后端重新拉取节点，以同步真实的 is_active 状态
      // （避免乐观更新与实际状态不一致的问题）
      await fetchGroupNodes(groupTag);
    }
    return res;
  }

  async function changeProxyMode(mode: "global" | "rule" | "direct") {
    const res = await setProxyMode(mode);
    if (res.success) {
      proxyMode.value = mode;
    } else {
      toast.error("切换代理模式失败", "请确认 Sing-box 核心是否在正常运行。");
    }
    return res;
  }

  return {
    groups,
    nodeMap,
    proxyMode,
    loading,
    error,
    activeGroup,
    recentGroups,
    fetchGroups,
    fetchGroupNodes,
    selectNode,
    changeProxyMode,
    recordGroupUsage,
  };
});
