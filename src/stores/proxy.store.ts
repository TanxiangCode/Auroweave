/**
 * Pinia Store — 代理节点与分组
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { ProxyGroup, ProxyNode, CustomGroupRule } from "@/types";
import { getProxyGroups, getGroupNodes, selectGroupNode, setProxyMode, getProxyMode, updateGroupConfig as updateGroupConfigApi } from "@/api/ipc/proxy";
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

  // ---- 自定义分组规则（从 localStorage 持久化） ----
  const customGroupRules = ref<CustomGroupRule[]>([]);

  function loadCustomGroupRules() {
    try {
      const stored = localStorage.getItem("auroweave_custom_group_rules");
      if (stored) {
        customGroupRules.value = JSON.parse(stored);
      }
    } catch (e) {
      console.error("加载自定义分组规则失败:", e);
    }
  }

  function saveCustomGroupRules() {
    localStorage.setItem("auroweave_custom_group_rules", JSON.stringify(customGroupRules.value));
  }

  function addCustomGroupRule(rule: Omit<CustomGroupRule, "id">) {
    const newRule: CustomGroupRule = {
      ...rule,
      id: Date.now().toString(),
    };
    customGroupRules.value.push(newRule);
    customGroupRules.value.sort((a, b) => a.order - b.order);
    saveCustomGroupRules();
  }

  function updateCustomGroupRule(rule: CustomGroupRule) {
    const idx = customGroupRules.value.findIndex((r) => r.id === rule.id);
    if (idx !== -1) {
      customGroupRules.value[idx] = rule;
      customGroupRules.value.sort((a, b) => a.order - b.order);
      saveCustomGroupRules();
    }
  }

  function deleteCustomGroupRule(id: string) {
    customGroupRules.value = customGroupRules.value.filter((r) => r.id !== id);
    saveCustomGroupRules();
  }

  /** 根据自定义规则对所有节点进行分组返回 */
  function applyCustomGroups(allNodes: ProxyNode[]): Map<string, ProxyNode[]> {
    if (customGroupRules.value.length === 0) {
      return new Map();
    }

    const result = new Map<string, ProxyNode[]>();
    const matched = new Set<string>();

    for (const rule of customGroupRules.value) {
      if (!rule.enabled) continue;

      const matchedNodes: ProxyNode[] = [];
      for (const node of allNodes) {
        if (matched.has(node.tag)) continue;

        let is_match = false;
        if (rule.match_type === "keyword") {
          is_match = rule.keywords.some((kw) =>
            node.tag.toLowerCase().includes(kw.toLowerCase())
          );
        } else if (rule.match_type === "regex") {
          try {
            const regex = new RegExp(rule.pattern, "i");
            is_match = regex.test(node.tag);
          } catch (e) {
            console.warn("无效的正则表达式:", rule.pattern, e);
          }
        } else if (rule.match_type === "protocol") {
          is_match = rule.protocols.includes(node.type.toLowerCase());
        }

        if (is_match) {
          matchedNodes.push(node);
          matched.add(node.tag);
        }
      }

      if (matchedNodes.length > 0) {
        result.set(rule.name, matchedNodes);
      }
    }

    // 未匹配的节点放入"其他"分组
    const unmatched = allNodes.filter((n) => !matched.has(n.tag));
    if (unmatched.length > 0) {
      result.set("其他", unmatched);
    }

    return result;
  }

  // 从 localStorage 获取使用计数，格式为 Record<string, number>
  // 带容错：存储损坏时回退空对象，避免 JSON.parse 抛错导致 store 初始化失败
  const groupUsage = ref<Record<string, number>>(
    (() => {
      try {
        return JSON.parse(localStorage.getItem("auroweave_group_usage") || "{}");
      } catch {
        return {};
      }
    })()
  );

  // 常用分组列表：按使用次数从大到小排序，只取使用次数 > 1 的分组，上限 RECENT_GROUPS_MAX
  const recentGroups = computed(() => {
    return Object.entries(groupUsage.value)
      .filter(([_, count]) => count > 1)
      .sort((a, b) => b[1] - a[1])
      .map(([tag]) => tag)
      .slice(0, RECENT_GROUPS_MAX);
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
      // 追溯到链路末端仍无节点名：说明该组尚未选出节点，如实显示"未知节点"
      const leafName = currentTag ? currentTag : "未知节点";
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
    try {
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

        // 清除过期的 nodeMap 缓存（订阅切换后节点会变化）
        const currentTags = new Set(sorted.map(g => g.tag));
        for (const tag of nodeMap.value.keys()) {
          if (!currentTags.has(tag)) {
            nodeMap.value.delete(tag);
          }
        }

        // 默认将初始活跃分组载入最近列表作为兜底展示
        const primary = sorted.find((g) => g.type === "selector");
        if (primary && Object.keys(groupUsage.value).length === 0) {
          groupUsage.value[primary.tag] = 2; // 兜底：主策略组初始有 2 次，算作常用
          localStorage.setItem("auroweave_group_usage", JSON.stringify(groupUsage.value));
        }
      } else {
        error.value = res.error ?? "获取分组失败";
      }
    } finally {
      loading.value = false;
    }
  }

  async function fetchGroupNodes(groupTag: string) {
    const res = await getGroupNodes(groupTag);
    if (res.success && res.data) {
      nodeMap.value.set(groupTag, res.data);
    }
  }

  /** 静默刷新分组列表（不触发 loading，用于定期更新 URLTest 组的 now 字段） */
  async function refreshGroups() {
    const res = await getProxyGroups();
    if (res.success && res.data) {
      const topTags = ["proxy", "auto", "balance"];
      const sorted = [...res.data].sort((a, b) => {
        const indexA = topTags.indexOf(a.tag);
        const indexB = topTags.indexOf(b.tag);
        if (indexA !== -1 && indexB !== -1) return indexA - indexB;
        if (indexA !== -1) return -1;
        if (indexB !== -1) return 1;
        return a.tag.localeCompare(b.tag, "zh-CN");
      });

      // 仅当分组结构变化时才更新（避免不必要的响应式触发）
      const oldTags = groups.value.map(g => g.tag).join(",");
      const newTags = sorted.map(g => g.tag).join(",");
      const structureChanged = oldTags !== newTags;

      groups.value = sorted;

      // 同步更新 nodeMap 中的 is_active 状态（基于最新的 now 字段）
      for (const group of sorted) {
        const nodes = nodeMap.value.get(group.tag);
        if (nodes && group.now) {
          const updated = nodes.map((n) => ({
            ...n,
            is_active: n.tag === group.now,
          }));
          // 仅当 is_active 有变化时才更新（减少不必要的 Map 重新赋值）
          const activeChanged = nodes.some((n, i) => n.is_active !== updated[i].is_active);
          if (activeChanged || structureChanged) {
            nodeMap.value.set(group.tag, updated);
          }
        }
      }

      // 分组结构变化时清除过期的 nodeMap 缓存
      if (structureChanged) {
        const currentTags = new Set(sorted.map(g => g.tag));
        for (const tag of nodeMap.value.keys()) {
          if (!currentTags.has(tag)) {
            nodeMap.value.delete(tag);
          }
        }
      }
    }
  }

  /** 清空代理数据缓存（订阅切换后调用） */
  function clearCache() {
    groups.value = [];
    nodeMap.value.clear();
  }

  async function selectNode(groupTag: string, nodeTag: string) {
    const group = groups.value.find((g) => g.tag === groupTag);
    if (group && group.type !== "selector") {
      return { success: false, error: "该策略组为自动或非手动选择类型，不支持手动切换节点", code: 400 };
    }
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

  // === 分组配置持久化（P0 修复新增）===
  // 调用 group_update_config IPC，将分组测速配置（interval/tolerance/url）持久化到后端。
  // interval 支持两种形式：字符串时长（"3m"）或秒数；tolerance 单位为毫秒。

  /** 将字符串时长（如 "3m"、"30s"、"1h"）解析为秒数 */
  function parseIntervalToSeconds(input: string): number | undefined {
    const m = input.trim().match(/^(\d+)\s*(s|m|h)?$/i);
    if (!m) return undefined;
    const val = parseInt(m[1], 10);
    const unit = (m[2] || "s").toLowerCase();
    if (unit === "m") return val * 60;
    if (unit === "h") return val * 3600;
    return val;
  }

  /** 更新分组测速配置（纯新增：编辑弹窗保存） */
  async function updateGroupConfig(
    groupTag: string,
    config: { interval?: number | string; tolerance?: number; url?: string }
  ) {
    const normalized: { interval?: number; tolerance?: number; url?: string } = {};
    if (config.interval !== undefined && config.interval !== "") {
      normalized.interval =
        typeof config.interval === "number"
          ? config.interval
          : parseIntervalToSeconds(String(config.interval));
    }
    if (config.tolerance !== undefined && config.tolerance !== null) {
      normalized.tolerance = Number(config.tolerance);
    }
    if (config.url !== undefined && config.url !== null) {
      normalized.url = String(config.url).trim();
    }
    return updateGroupConfigApi(groupTag, normalized);
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
    customGroupRules,
    fetchGroups,
    fetchGroupNodes,
    refreshGroups,
    clearCache,
    selectNode,
    changeProxyMode,
    recordGroupUsage,
    loadCustomGroupRules,
    addCustomGroupRule,
    updateCustomGroupRule,
    deleteCustomGroupRule,
    applyCustomGroups,
    // 分组配置持久化（P0 修复新增）
    updateGroupConfig,
  };
});
