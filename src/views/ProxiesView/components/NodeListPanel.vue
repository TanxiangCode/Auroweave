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
    /** 节点列表（已搜索+筛选+排序） */
    nodes: ProxyNode[];
    /** 原始节点数量（用于判断是否为空） */
    rawCount: number;
    /** 是否加载中 */
    loading: boolean;
    /** 当前搜索关键词 */
    searchText: string;
    /** 当前解锁服务筛选（"gemini:yes" 形态；空串=未筛选） */
    unlockFilter?: string;
    /** 是否隐藏已经测出超时的节点 */
    hideTimedOut?: boolean;
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
    unlockFilter: "",
    hideTimedOut: false,
    fetchError: null,
    pinnedSet: () => new Set<string>(),
  }
);

const emit = defineEmits<{
  select: [nodeTag: string];
  'test-latency': [nodeTag: string];
  'test-speed': [nodeTag: string];
  'check-unlock': [nodeTag: string];
  'toggle-pin': [nodeTag: string];
  'refresh-groups': [];
}>();

const speedtestStore = useSpeedtestStore();
</script>

<template>
  <!-- 加载中（首分支：v-if 锚点，加载完成前不闪错误/空态） -->
  <div v-if="loading" class="state-tip">
    <div class="state-inner">
      <span class="loading-spin"></span>
      <span>正在加载代理节点列表...</span>
    </div>
  </div>

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

  <!-- 筛选/搜索无匹配（两种来源文案区分，避免误导性提示） -->
  <EmptyState
    v-else-if="nodes.length === 0"
    icon="Search"
    :title="unlockFilter || hideTimedOut ? '当前筛选下无匹配节点' : '没有找到匹配的节点'"
    :description="unlockFilter
      ? '解锁筛选未命中任何节点——可能是尚未检测，试试清除筛选或先跑一轮解锁检测'
      : hideTimedOut
        ? '当前没有可显示的节点——已测节点可能全部超时，未测试节点仍会保留'
        : `搜索「${searchText}」无结果，试试其他关键词`"
  />

  <!-- 节点列表 / 网格容器：data-node-tag 供"定位当前节点"滚动选择器使用 -->
  <div v-else class="nodes-scroll">
    <div class="nodes-container" :class="layoutMode">
      <div
        v-for="node in nodes"
        :key="node.tag"
        class="node-slot"
        :data-node-tag="node.tag"
      >
        <NodeCard
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
          @check-unlock="emit('check-unlock', node.tag)"
          @toggle-pin="emit('toggle-pin', node.tag)"
        />
      </div>
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

/* 大节点量保险（O-8 实测决策）：300 节点 ≈5100 DOM 全量渲染可接受，
   content-visibility 让视口外卡片跳过 layout/paint——组件级虚拟化的
   零成本替代（contain-intrinsic-size 保留占位防滚动跳动） */
.nodes-container.grid > * {
  content-visibility: auto;
  contain-intrinsic-size: auto 90px;
}

/* List 紧凑列表布局 */
.nodes-container.list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* 定位标记插槽：不引入额外布局盒子 */
.node-slot {
  display: contents;
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

