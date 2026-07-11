/**
 * Pinia Store — 代理节点与分组
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { ProxyGroup, ProxyNode } from "@/types";
import { getProxyGroups, getGroupNodes, selectGroupNode, setProxyMode, getProxyMode } from "@/api/ipc/proxy";
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

  /** 递归物理工作节点追溯计算属性，解决负载均衡与自动组的显示盲区 */
  const workingNodeName = computed(() => {
    const mainGroup = groups.value.find((g) => g.tag === "proxy");
    if (!mainGroup) return "直连";

    let currentTag = mainGroup.now;
    if (!currentTag) return "直连";

    if (currentTag.toLowerCase() === "direct") return "直连";

    let depth = 0;
    const path: string[] = [];

    while (depth < 5) {
      const subGroup = groups.value.find((g) => g.tag === currentTag);
      if (subGroup) {
        path.push(currentTag);
        currentTag = subGroup.now || "";
        depth++;
      } else {
        break;
      }
    }

    if (path.length > 0) {
      const groupLabel = path[0] === "balance"
        ? "负载均衡"
        : path[0] === "auto"
        ? "自动选择"
        : path[0];
      const leafName = currentTag ? currentTag : "测速中...";
      return `${groupLabel} (${leafName})`;
    }

    return currentTag;
  });

  // ---- 动作 ----
  async function syncProxyMode() {
    const res = await getProxyMode();
    if (res.success && res.data) {
      proxyMode.value = res.data.toLowerCase() as any;
    } else {
      proxyMode.value = "direct";
    }
  }

  async function fetchGroups() {
    await syncProxyMode();
    loading.value = true;
    error.value = null;
    const res = await getProxyGroups();
    if (res.success && res.data) {
      // 置顶排序逻辑：将 proxy、auto、balance 置顶，其他按字母表排序
      const topTags = ["proxy", "auto", "balance"];
      const sorted = [...res.data].sort((a, b) => {
        const indexA = topTags.indexOf(a.tag);
        const indexB = topTags.indexOf(b.tag);
        if (indexA !== -1 && indexB !== -1) {
          return indexA - indexB;
        }
        if (indexA !== -1) return -1;
        if (indexB !== -1) return 1;
        return a.tag.localeCompare(b.tag, "zh-CN");
      });
      groups.value = sorted;

      // 默认将初始活跃分组载入最近列表作为兜底展示
      const primary = sorted.find((g) => g.type === "selector");
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
    console.log('selectNode')
    const group = groups.value.find((g) => g.tag === groupTag);
    if (group && group.type !== "selector") {
      return { success: false, error: "该策略组为自动或非手动选择类型，不支持手动切换节点", code: 400 };
    }
    const res = await selectGroupNode(groupTag, nodeTag);
    console.log(res)
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
    workingNodeName,
    recentGroups,
    fetchGroups,
    fetchGroupNodes,
    selectNode,
    changeProxyMode,
    recordGroupUsage,
  };
});
