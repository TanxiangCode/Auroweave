<script setup lang="ts">
/**
 * 节点列表面板
 * 作者: TanXiang
 *
 * 展示当前分组的节点列表，包含 loading / empty / no-match 三种状态
 */
import NodeCard from "@/components/proxy/NodeCard.vue";
import SvgIcon from "@/components/common/SvgIcon.vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import type { ProxyNode } from "@/types";

const props = defineProps<{
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
}>();

const emit = defineEmits<{
  select: [nodeTag: string];
  'test-latency': [nodeTag: string];
  'test-speed': [nodeTag: string];
}>();

const speedtestStore = useSpeedtestStore();
</script>

<template>
  <!-- 加载中 -->
  <div v-if="loading" class="state-tip">
    ⏳ 正在加载节点列表...
  </div>

  <!-- 暂无节点 -->
  <div v-else-if="rawCount === 0" class="state-tip">
    📭 暂无节点数据，请点击刷新
  </div>

  <!-- 搜索无匹配 -->
  <div v-else-if="nodes.length === 0" class="state-tip">
    🔍 没有匹配「{{ searchText }}」的节点
  </div>

  <!-- 节点列表 -->
  <div v-else class="nodes-scroll">
    <div class="nodes-list">
      <NodeCard
        v-for="node in nodes"
        :key="node.tag"
        :node-tag="node.tag"
        :node-type="node.type"
        :is-active="node.is_active"
        :latency="speedtestStore.latencyMap[node.tag]"
        :speed-bps="speedtestStore.throughputMap[node.tag]?.download_bps"
        :is-selectable="isSelectable"
        @select="emit('select', node.tag)"
        @test-latency="emit('test-latency', node.tag)"
        @test-speed="emit('test-speed', node.tag)"
      />
    </div>
  </div>
</template>

<style scoped>
.nodes-scroll {
  flex: 1;
  overflow-y: auto;
  margin-top: var(--space-3);
  padding-right: var(--space-1);
}

.nodes-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.state-tip {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
</style>
