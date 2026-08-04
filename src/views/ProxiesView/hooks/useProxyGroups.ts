/**
 * 代理分组选择与路由链路计算 Hook
 * 作者: TanXiang
 *
 * 职责：管理分组选择状态、系统/地区分组分类、路由链路追踪
 */
import { ref, computed } from "vue";
import { useRoute } from "vue-router";
import { useProxyStore } from "@/stores/proxy.store";
import { storeToRefs } from "pinia";
import type { ProxyGroup, ProxyNode } from "@/types";

/** 系统内置分组标签（主策略组） */
const systemGroupTags = ["proxy", "auto", "balance"];

/**
 * 代理分组管理 Hook
 *
 * 提供分组选择、系统/地区分类、路由链路计算等能力
 */
export function useProxyGroups() {
  const proxyStore = useProxyStore();
  const route = useRoute();

  const { groups, loading } = storeToRefs(proxyStore);

  const selectedGroupTag = ref<string>("");

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

  /** 当前选中的分组对象 */
  const currentGroup = computed<ProxyGroup | undefined>(() => {
    return groups.value.find((x) => x.tag === selectedGroupTag.value);
  });

  /** 当前分组是否为手动选择类型 */
  const isSelectorGroup = computed(() => {
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

  /** 当前分组的原始节点列表（未经搜索/排序） */
  const rawNodes = computed<ProxyNode[]>(() => {
    const nodes = proxyStore.nodeMap.get(selectedGroupTag.value);
    return nodes ?? [];
  });

  /** 初始化：从 URL query 或首个分组确定选中分组 */
  async function initSelectedGroup() {
    if (groups.value.length > 0) {
      const qGroup = route.query.group as string;
      const exists = groups.value.some((g) => g.tag === qGroup);
      selectedGroupTag.value = exists ? qGroup : groups.value[0].tag;
      await proxyStore.fetchGroupNodes(selectedGroupTag.value);
    }
  }

  /** 切换分组 */
  async function handleGroupSelect(groupTag: string) {
    selectedGroupTag.value = groupTag;
    await proxyStore.fetchGroupNodes(groupTag);
    proxyStore.recordGroupUsage(groupTag);
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
    currentGroup,
    isSelectorGroup,
    routingGroupTags,
    rawNodes,
    // 方法
    initSelectedGroup,
    handleGroupSelect,
  };
}
