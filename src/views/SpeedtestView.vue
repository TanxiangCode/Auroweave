<script setup lang="ts">
/**
 * 智能测速大厅
 * 作者: TanXiang
 */
import { ref, computed, onMounted } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useToast } from "@/composables/useToast";
import NodeCard from "@/components/proxy/NodeCard.vue";

const proxyStore = useProxyStore();
const speedtestStore = useSpeedtestStore();
const toast = useToast();

const activeGroupTag = ref<string>("");
const showConfirmModal = ref(false);

onMounted(async () => {
  await proxyStore.fetchGroups();
  await speedtestStore.init();
  if (proxyStore.groups.length > 0) {
    activeGroupTag.value = proxyStore.groups[0].tag;
  }
});

const currentNodes = computed(() => {
  const g = proxyStore.groups.find((x) => x.tag === activeGroupTag.value);
  return g ? g.proxies : [];
});

const activeGroupNow = computed(() => {
  const g = proxyStore.groups.find((x) => x.tag === activeGroupTag.value);
  return g?.now || "";
});

async function handleSelectNode(nodeTag: string) {
  if (!activeGroupTag.value) return;
  await proxyStore.selectNode(activeGroupTag.value, nodeTag);
  toast.success("节点已切换", `切至: ${nodeTag}`);
}

async function handleRunLatency() {
  if (!activeGroupTag.value) return;
  toast.info("正在并发测试延迟...");
  await speedtestStore.testLatency(activeGroupTag.value, currentNodes.value);
  toast.success("延迟测试完成");
}

async function handleSingleLatency(nodeTag: string) {
  if (!activeGroupTag.value) return;
  await speedtestStore.testLatency(activeGroupTag.value, [nodeTag]);
}

async function handleSingleSpeed(nodeTag: string) {
  toast.info("开始节点吞吐量测试", `正在测试: ${nodeTag}`);
  const res = await speedtestStore.testSingleThroughput(nodeTag);
  if (res.success && res.data) {
    const mbps = (res.data.download_bps / (1024 * 1024)).toFixed(1);
    toast.success("单节点测速完成", `${nodeTag}: ${mbps} MB/s`);
  } else {
    toast.error("测速失败", res.error);
  }
}

async function confirmBatchSpeedTest() {
  showConfirmModal.value = false;
  if (!activeGroupTag.value) return;
  await speedtestStore.startBatchTest(activeGroupTag.value, currentNodes.value);
  toast.info("已启动批量串行测速任务");
}
</script>

<template>
  <div class="speedtest-view">
    <header class="page-header">
      <div class="title-area">
        <h1>📊 智能测速大厅</h1>
        <p class="subtitle">实时测量全量节点延迟与吞吐能力，助力选出最佳节点</p>
      </div>
      <div class="header-actions">
        <button class="btn primary" @click="handleRunLatency">⚡ 批量测延迟</button>
        <button class="btn secondary" @click="showConfirmModal = true">📶 批量吞吐量测速</button>
      </div>
    </header>

    <!-- 批量测速进度条 -->
    <div
      v-if="speedtestStore.isBatchTesting && speedtestStore.batchProgress"
      class="batch-progress-card glass-effect"
    >
      <div class="progress-info">
        <span>正在测速: <strong>{{ speedtestStore.batchProgress.current_node }}</strong></span>
        <span>进度: {{ speedtestStore.batchProgress.current_index }} / {{ speedtestStore.batchProgress.total }}</span>
      </div>
      <div class="progress-bar-bg">
        <div
          class="progress-bar-fill"
          :style="{ width: `${(speedtestStore.batchProgress.current_index / speedtestStore.batchProgress.total) * 100}%` }"
        ></div>
      </div>
      <button class="btn-cancel" @click="speedtestStore.cancelBatch">取消测速</button>
    </div>

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
          <h3>⚠️ 批量吞吐量测速确认</h3>
          <p>将对分组 <strong>「{{ activeGroupTag }}」</strong> 的 <strong>{{ currentNodes.length }}</strong> 个节点依次进行带宽测试。</p>
          <div class="estimate-box">
            <div>⏱️ 预计耗时: 约 {{ Math.ceil(currentNodes.length * 3 / 60) }} 分钟</div>
            <div>📉 预计流量消耗: 约 {{ currentNodes.length * 15 }} MB</div>
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

/* 批量进度卡片 */
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

/* 节点网格 */
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

/* 二次按钮变体 */
.btn.secondary {
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  color: var(--text-primary);
}

/* 弹窗内元素 */
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
