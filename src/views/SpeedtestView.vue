<template>
  <div class="speedtest-view">
    <header class="speedtest-header">
      <div class="title-area">
        <h1>📊 智能测速大厅</h1>
        <p class="subtitle">实时测量全量节点延迟与吞吐能力，助力选出最佳节点</p>
      </div>

      <div class="header-actions">
        <button class="btn primary" @click="handleRunLatency">
          ⚡ 批量测延迟
        </button>
        <button class="btn secondary" @click="showConfirmModal = true">
          📶 批量吞吐量测速
        </button>
      </div>
    </header>

    <!-- 批量测速进度条 -->
    <div v-if="speedtestStore.isBatchTesting && speedtestStore.batchProgress" class="batch-progress-card glass-effect">
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
      <div class="group-tabs">
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

<script setup lang="ts">
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

<style scoped>
.speedtest-view {
  padding: 48px 24px 24px 24px;
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.speedtest-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title-area h1 {
  font-size: 20px;
  font-weight: 700;
}

.subtitle {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
  margin-top: 4px;
}

.header-actions {
  display: flex;
  gap: 12px;
}

.btn {
  padding: 8px 16px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  border: none;
  transition: all 0.2s ease;
}

.btn.primary {
  background: linear-gradient(135deg, #00f2fe, #4facfe);
  color: #000;
}

.btn.secondary {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.12);
  color: rgba(255, 255, 255, 0.9);
}

.btn:hover {
  transform: translateY(-1px);
  filter: brightness(1.1);
}

.batch-progress-card {
  padding: 16px 20px;
  background: rgba(0, 242, 254, 0.08);
  border: 1px solid rgba(0, 242, 254, 0.25);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
}

.progress-bar-bg {
  height: 6px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 3px;
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: #00f2fe;
  transition: width 0.3s ease;
}

.btn-cancel {
  align-self: flex-end;
  background: transparent;
  border: none;
  color: #f87171;
  font-size: 12px;
  cursor: pointer;
}

.group-tabs {
  display: flex;
  gap: 8px;
  margin-bottom: 16px;
}

.tab-btn {
  padding: 6px 14px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
  cursor: pointer;
}

.tab-btn.active {
  background: rgba(0, 242, 254, 0.15);
  border-color: #00f2fe;
  color: #fff;
  font-weight: 600;
}

.node-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 16px;
}

.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 99999;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(12px);
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-card {
  width: 440px;
  padding: 24px;
  background: rgba(18, 22, 34, 0.95);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 16px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.estimate-box {
  background: rgba(255, 255, 255, 0.04);
  padding: 12px;
  border-radius: 8px;
  font-size: 13px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.warning-tip {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 8px;
}
</style>
