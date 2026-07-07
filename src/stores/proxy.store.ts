/**
 * Pinia Store — 代理节点与分组
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { ProxyGroup, ProxyNode } from "@/types";
import { getProxyGroups, getGroupNodes, selectGroupNode, setProxyMode } from "@/api/ipc/proxy";
import { RECENT_GROUPS_MAX } from "@/constants";
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

  // 最近选择的分组列表，上限为 RECENT_GROUPS_MAX
  const recentGroups = ref<string[]>([]);

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
      if (primary && recentGroups.value.length === 0) {
        recentGroups.value.push(primary.tag);
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
      // 乐观更新本地状态
      const group = groups.value.find((g) => g.tag === groupTag);
      if (group) group.now = nodeTag;

      // 更新最近选择的分组列表并限制长度为 RECENT_GROUPS_MAX
      const index = recentGroups.value.indexOf(groupTag);
      if (index !== -1) {
        recentGroups.value.splice(index, 1);
      }
      recentGroups.value.unshift(groupTag);
      if (recentGroups.value.length > RECENT_GROUPS_MAX) {
        recentGroups.value = recentGroups.value.slice(0, RECENT_GROUPS_MAX);
      }
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
  };
});
