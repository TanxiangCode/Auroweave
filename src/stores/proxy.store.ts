/**
 * Pinia Store — 代理节点与分组
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { ProxyGroup, ProxyNode } from "@/types";
import { getProxyGroups, getGroupNodes, selectGroupNode, setProxyMode } from "@/api/ipc/proxy";

export const useProxyStore = defineStore("proxy", () => {
  // ---- 状态 ----
  const groups = ref<ProxyGroup[]>([]);
  /** 各分组的节点缓存 key=groupTag */
  const nodeMap = ref<Map<string, ProxyNode[]>>(new Map());
  const proxyMode = ref<"global" | "rule" | "direct">("rule");
  const loading = ref(false);
  const error = ref<string | null>(null);

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
      // 乐观更新本地状态
      const group = groups.value.find((g) => g.tag === groupTag);
      if (group) group.now = nodeTag;
    }
    return res;
  }

  async function changeProxyMode(mode: "global" | "rule" | "direct") {
    const res = await setProxyMode(mode);
    if (res.success) {
      proxyMode.value = mode;
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
    fetchGroups,
    fetchGroupNodes,
    selectNode,
    changeProxyMode,
  };
});
