/**
 * 代理分组选择与路由链路计算 Hook
 * 作者: TanXiang
 *
 * 职责：管理分组选择状态、系统/地区分组分类、自定义虚拟分组、路由链路追踪
 */
import { ref, computed } from "vue";
import { useRoute } from "vue-router";
import { useProxyStore } from "@/stores/proxy.store";
import { storeToRefs } from "pinia";
import type { ProxyGroup, ProxyNode } from "@/types";

/** 系统内置分组标签（主策略组） */
const systemGroupTags = ["proxy", "auto", "balance"];

/** 自定义虚拟分组前缀（避免与内核真实分组 tag 撞名） */
export const CUSTOM_GROUP_PREFIX = "custom:";

/**
 * 代理分组管理 Hook
 *
 * 提供分组选择、系统/地区分类、自定义虚拟分组、路由链路计算等能力
 */
export function useProxyGroups() {
  const proxyStore = useProxyStore();
  const route = useRoute();

  const { groups, loading } = storeToRefs(proxyStore);

  const selectedGroupTag = ref<string>("");
  /** 分组切换令牌：快速连点时，仅最后一次切换的 fetch 允许收尾，避免旧响应错序 */
  let selectionToken = 0;

  /** 系统内置分组（主策略组：proxy / auto / balance） */
  const systemGroups = computed<ProxyGroup[]>(() => {
    return groups.value.filter((g) => systemGroupTags.includes(g.tag));
  });

  /** 地区分组（非系统分组的 urltest 类型） */
  const regionGroups = computed<ProxyGroup[]>(() => {
    return groups.value.filter(
      (g) => !systemGroupTags.includes(g.tag) && g.type === "urltest"
    );
  });

  /** 自定义分组当前是否被选中 */
  const isCustomGroup = computed(
    () => selectedGroupTag.value.startsWith(CUSTOM_GROUP_PREFIX)
  );

  /**
   * 自定义虚拟分组视图：把匹配结果包装成 ProxyGroup 形态供 GroupSidebar 展示
   * （tag 为 custom:{规则名}，前端本地聚合不进内核）
   */
  const customGroups = computed<ProxyGroup[]>(() => {
    if (proxyStore.customGroupRules.length === 0) return [];
    // 全量节点池 = 主 selector 组的全部成员（自定义规则对该池做匹配）
    const pool = proxyStore.nodeMap.get("proxy") ?? [];
    if (pool.length === 0) return [];
    const grouped = proxyStore.applyCustomGroups(pool);
    const out: ProxyGroup[] = [];
    for (const rule of proxyStore.customGroupRules) {
      if (!rule.enabled) continue;
      // "其他" 分桶不属于任何规则，跳过
      const nodes = grouped.get(rule.name);
      if (!nodes) continue;
      out.push({
        tag: `${CUSTOM_GROUP_PREFIX}${rule.name}`,
        type: "selector", // 仅用于展示层徽章；不可真实切换内核选择
        proxies: nodes.map((n) => n.tag),
        now: nodes.length > 0 ? `${nodes.length} 个节点` : undefined,
      });
    }
    return out;
  });

  /** 当前选中的分组对象（自定义虚拟分组时合成一个） */
  const currentGroup = computed<ProxyGroup | undefined>(() => {
    if (isCustomGroup.value) {
      return customGroups.value.find(
        (g) => g.tag === selectedGroupTag.value
      );
    }
    return groups.value.find((x) => x.tag === selectedGroupTag.value);
  });

  /** 当前分组是否为手动选择类型（自定义虚拟分组恒不可切换内核选择） */
  const isSelectorGroup = computed(() => {
    if (isCustomGroup.value) return false;
    return currentGroup.value?.type === "selector";
  });

  /**
   * 计算当前处于激活出口链路上的所有策略组 tag
   * 追踪主 selector 分组的 now 链路，最多追溯 10 层
   */
  const routingGroupTags = computed<Set<string>>(() => {
    const tags = new Set<string>();
    const primary = groups.value.find((g) => g.type === "selector");
    if (!primary) return tags;

    tags.add(primary.tag);
    let currentTagName = primary.now;
    for (let i = 0; i < 10 && currentTagName; i++) {
      const nextGroup = groups.value.find((g) => g.tag === currentTagName);
      if (nextGroup) {
        tags.add(nextGroup.tag);
        currentTagName = nextGroup.now;
      } else {
        break;
      }
    }
    return tags;
  });

  /** 当前分组的原始节点列表（未经搜索/排序；自定义分组走本地匹配结果） */
  const rawNodes = computed<ProxyNode[]>(() => {
    if (isCustomGroup.value) {
      const group = customGroups.value.find(
        (g) => g.tag === selectedGroupTag.value
      );
      if (!group) return [];
      const pool = proxyStore.nodeMap.get("proxy") ?? [];
      const tags = new Set(group.proxies);
      return pool.filter((n) => tags.has(n.tag));
    }
    const nodes = proxyStore.nodeMap.get(selectedGroupTag.value);
    return nodes ?? [];
  });

  /** 初始化：从 URL query 或首个分组确定选中分组，保留已有选择 */
  async function initSelectedGroup() {
    if (groups.value.length > 0) {
      const qGroup = route.query.group as string;
      const qExists = qGroup && groups.value.some((g) => g.tag === qGroup);
      const currentExists = selectedGroupTag.value &&
        (isCustomGroup.value
          ? customGroups.value.some((g) => g.tag === selectedGroupTag.value)
          : groups.value.some((g) => g.tag === selectedGroupTag.value));

      if (qExists) {
        selectedGroupTag.value = qGroup;
      } else if (currentExists) {
        // 保留当前选中分组（订阅刷新后分组结构不变时避免跳回第一个）
      } else {
        selectedGroupTag.value = groups.value[0].tag;
      }
      if (!isCustomGroup.value) {
        await proxyStore.fetchGroupNodes(selectedGroupTag.value);
      }
    }
  }

  /** 切换分组（带竞态保护：快速连点时仅最后一次切换生效；虚拟分组不发内核请求） */
  async function handleGroupSelect(groupTag: string) {
    selectedGroupTag.value = groupTag;
    if (isCustomGroup.value) return; // 本地匹配，无需拉取
    const token = ++selectionToken;
    try {
      await proxyStore.fetchGroupNodes(groupTag);
    } finally {
      // 仅当本次 fetch 仍是用户最后一次选择时才记录使用频次，
      // 防止旧请求返回后把过期分组的 usage 记入统计
      if (token === selectionToken && selectedGroupTag.value === groupTag) {
        proxyStore.recordGroupUsage(groupTag);
      }
    }
  }

  return {
    // 状态
    groups,
    loading,
    selectedGroupTag,
    systemGroupTags,
    // 计算属性
    systemGroups,
    regionGroups,
    customGroups,
    isCustomGroup,
    currentGroup,
    isSelectorGroup,
    routingGroupTags,
    rawNodes,
    // 方法
    initSelectedGroup,
    handleGroupSelect,
  };
}
