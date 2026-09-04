<script setup lang="ts">
/**
 * 智能测速大厅 — 主入口
 * 作者: TanXiang
 *
 * 职责：布局拼装、状态绑定
 */
import { onMounted } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import BatchProgressCard from "@/components/speedtest/BatchProgressCard.vue";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import NodeCard from "@/components/proxy/NodeCard.vue";
import { useSpeedtest } from "./hooks/useSpeedtest";

const speedtestStore = useSpeedtestStore();

const {
  activeGroupTag,
  showConfirmModal,
  currentNodes,
  activeGroupNow,
  init,
  handleSelectNode,
  handleRunLatency,
  handleSingleLatency,
  handleSingleSpeed,
  confirmBatchSpeedTest,
  proxyStore,
} = useSpeedtest();

onMounted(() => {
  init();
});
</script>

<template>
  <div class="speedtest-view">
    <header class="page-header">
      <div class="title-area">
        <h1><BaseIcon name="Gauge" :size="24" class="title-icon" /> 智能测速大厅</h1>
        <p class="subtitle">实时测量全量节点延迟与吞吐能力，助力选出最佳节点</p>
      </div>
      <div class="header-actions">
        <button class="btn primary" @click="handleRunLatency"><BaseIcon name="Zap" :size="15" /> 批量测延迟</button>
        <button class="btn secondary" @click="showConfirmModal = true"><BaseIcon name="Activity" :size="15" /> 批量吞吐量测速</button>
      </div>
    </header>

    <!-- 批量测速进度条（共享组件） -->
    <BatchProgressCard
      :visible="speedtestStore.isBatchTesting && !!speedtestStore.batchProgress"
      :progress="speedtestStore.batchProgress!"
      @cancel="speedtestStore.cancelBatch"
    />

    <!-- 节点分组与列表 -->
    <div class="nodes-section">
      <div class="tab-group">
        <button
          v-for="group in proxyStore.groups"
          :key="group.tag"
          class="tab-btn"
          :class="{ active: group.tag === activeGroupTag }"
          @click="activeGroupTag = group.tag"
        >
          {{ group.tag }}
        </button>
      </div>

      <div class="node-grid">
        <NodeCard
          v-for="nodeTag in currentNodes"
          :key="nodeTag"
          :node-tag="nodeTag"
          node-type="NODE"
          :is-active="nodeTag === activeGroupNow"
          :latency="speedtestStore.latencyMap[nodeTag]"
          :speed-bps="speedtestStore.throughputMap[nodeTag]?.download_bps"
          @select="handleSelectNode(nodeTag)"
          @test-latency="handleSingleLatency(nodeTag)"
          @test-speed="handleSingleSpeed(nodeTag)"
        />
      </div>
    </div>

    <!-- 批量测速确认 Modal -->
    <Teleport to="body">
      <div v-if="showConfirmModal" class="modal-backdrop" @click.self="showConfirmModal = false">
        <div class="modal-card glass-effect">
          <h3><BaseIcon name="AlertTriangle" :size="20" color="#f59e0b" /> 批量吞吐量测速确认</h3>
          <p>将对分组 <strong>「{{ activeGroupTag }}」</strong> 的 <strong>{{ currentNodes.length }}</strong> 个节点依次进行带宽测试。</p>
          <div class="estimate-box">
            <div><BaseIcon name="Clock" :size="14" /> 预计耗时: 约 {{ Math.ceil(currentNodes.length * speedtestStore.THROUGHPUT_TEST_DURATION_SEC / 60) }} 分钟</div>
            <div><BaseIcon name="TrendingDown" :size="14" /> 预计流量消耗: 约 {{ currentNodes.length * (speedtestStore.THROUGHPUT_TEST_CHUNK_BYTES / (1024 * 1024)) }} MB</div>
          </div>
          <p class="warning-tip">测速过程将以串行队列运行，以确保带宽测试结果精准无干扰。</p>
          <div class="modal-actions">
            <button class="btn text" @click="showConfirmModal = false">取消</button>
            <button class="btn primary" @click="confirmBatchSpeedTest">开始测速</button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.speedtest-view {
  padding: 48px var(--space-5) var(--space-5) var(--space-5);
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.batch-progress-card {
  padding: var(--space-4) var(--space-5);
  background: var(--accent-cyan-glow);
  border: 1px solid var(--border-accent);
  border-radius: var(--radius-md);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.btn-cancel {
  align-self: flex-end;
  background: transparent;
  border: none;
  color: var(--status-danger);
  font-size: var(--text-xs);
  cursor: pointer;
}

.nodes-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.node-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: var(--space-4);
}

.btn.secondary {
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  color: var(--text-primary);
}

.estimate-box {
  background: var(--surface-raised);
  padding: var(--space-3);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.warning-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

h3 {
  font-size: var(--text-md);
  font-weight: var(--weight-bold);
}

p {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: var(--leading-normal);
}

p strong {
  color: var(--accent-cyan-vivid);
}
</style>
