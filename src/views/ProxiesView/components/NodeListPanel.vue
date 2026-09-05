<script setup lang="ts">
/**
 * 节点列表面板 (升级版)
 * 作者: TanXiang
 *
 * 支持 Grid (网格) / List (紧凑列表) 视图模式
 */
import NodeCard from "@/components/proxy/NodeCard.vue";
import EmptyState from "@/components/common/EmptyState.vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import type { ProxyNode } from "@/types";

withDefaults(
  defineProps<{
    /** 节点列表（已搜索+排序） */
    nodes: ProxyNode[];
    /** 原始节点数量（用于判断是否为空） */
    rawCount: number;
    /** 是否加载中 */
    loading: boolean;
    /** 当前搜索关键词 */
    searchText: string;
    /** 是否为手动选择分组 */
    isSelectable: boolean;
    /** 布局模式 */
    layoutMode?: "grid" | "list";
    /** 分组数据拉取错误信息（有值时显示错误态而非误导性的空态） */
    fetchError?: string | null;
    /** 置顶收藏节点集合（星标高亮） */
    pinnedSet?: Set<string>;
  }>(),
  {
    layoutMode: "grid",
    fetchError: null,
    pinnedSet: () => new Set<string>(),
  }
);

const emit = defineEmits<{
  select: [nodeTag: string];
  'test-latency': [nodeTag: string];
  'test-speed': [nodeTag: string];
  'toggle-pin': [nodeTag: string];
  'refresh-groups': [];
}>();

const speedtestStore = useSpeedtestStore();
</script>

<template>
  <!-- 拉取失败错误态（区别于空态：明确告知是获取失败，而非没有订阅） -->
  <EmptyState
    v-else-if="fetchError"
    icon="AlertTriangle"
    title="节点列表获取失败"
    :description="fetchError ?? undefined"
  />

  <!-- 暂无节点 -->
  <EmptyState
    v-else-if="rawCount === 0"
    icon="Database"
    title="暂无可用节点"
    description="请确保订阅已导入并点击刷新"
    cta-text="重新拉取分组"
    cta-icon="RefreshCw"
    @cta="emit('refresh-groups')"
  />

  <!-- 搜索无匹配 -->
  <EmptyState
    v-else-if="nodes.length === 0"
    icon="Search"
    title="没有找到匹配的节点"
    :description="`搜索「${searchText}」无结果，试试其他关键词`"
  />

  <!-- 加载中 -->
  <div v-else-if="loading" class="state-tip">
    <div class="state-inner">
      <span class="loading-spin"></span>
      <span>正在加载代理节点列表...</span>
    </div>
  </div>

  <!-- 节点列表 / 网格容器 -->
  <div v-else class="nodes-scroll">
    <div class="nodes-container" :class="layoutMode">
      <NodeCard
        v-for="node in nodes"
        :key="node.tag"
        :node-tag="node.tag"
        :node-type="node.type"
        :is-active="node.is_active"
        :latency="speedtestStore.latencyMap[node.tag]"
        :speed-bps="speedtestStore.throughputMap[node.tag]?.download_bps"
        :is-selectable="isSelectable"
        :layout-mode="layoutMode"
        :is-pinned="pinnedSet.has(node.tag)"
        @select="emit('select', node.tag)"
        @test-latency="emit('test-latency', node.tag)"
        @test-speed="emit('test-speed', node.tag)"
        @toggle-pin="emit('toggle-pin', node.tag)"
      />
    </div>
  </div>
</template>

<style scoped>
.nodes-scroll {
  flex: 1;
  overflow-y: auto;
  margin-top: var(--space-3);
  padding-right: 4px;
}

/* Grid 网格布局 */
.nodes-container.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 10px;
  align-content: start;
}

/* List 紧凑列表布局 */
.nodes-container.list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.state-tip {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.state-inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
}

.state-icon {
  font-size: 28px;
}

.loading-spin {
  font-size: 24px;
  display: inline-block;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>

