<script setup lang="ts">
/**
 * 代理节点视图 (集成节点选择大厅与智能测速子功能)
 * 作者: TanXiang
 */
import { onMounted, ref, computed } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useToast } from "@/composables/useToast";
import { storeToRefs } from "pinia";
import NodeCard from "@/components/proxy/NodeCard.vue";

const proxyStore = useProxyStore();
const speedtestStore = useSpeedtestStore();
const toast = useToast();

const { groups, loading, error } = storeToRefs(proxyStore);
const selectedGroupTag = ref<string>("");
const viewMode = ref<"nodes" | "speedtest">("nodes");
const showConfirmModal = ref(false);

onMounted(async () => {
  await proxyStore.fetchGroups();
  await speedtestStore.init();
  if (groups.value.length > 0) {
    selectedGroupTag.value = groups.value[0].tag;
    await proxyStore.fetchGroupNodes(selectedGroupTag.value);
  }
});

const currentNodes = computed(() => {
  const g = groups.value.find((x) => x.tag === selectedGroupTag.value);
  return g ? g.proxies : [];
});

async function handleGroupSelect(groupTag: string) {
  selectedGroupTag.value = groupTag;
  await proxyStore.fetchGroupNodes(groupTag);

  // 记录到最近列表，采用 store 定义的限制规则
  const index = proxyStore.recentGroups.indexOf(groupTag);
  if (index !== -1) {
    proxyStore.recentGroups.splice(index, 1);
  }
  proxyStore.recentGroups.unshift(groupTag);
  if (proxyStore.recentGroups.length > 4) {
    proxyStore.recentGroups = proxyStore.recentGroups.slice(0, 4);
  }
}

async function handleNodeSelect(nodeTag: string) {
  if (!selectedGroupTag.value) return;
  await proxyStore.selectNode(selectedGroupTag.value, nodeTag);
  toast.success("节点已切换", `切至: ${nodeTag}`);
}

async function handleRunLatency() {
  if (!selectedGroupTag.value) return;
  toast.info("正在并发测试延迟...");
  await speedtestStore.testLatency(selectedGroupTag.value, currentNodes.value);
  toast.success("延迟测试完成");
}

async function handleSingleLatency(nodeTag: string) {
  if (!selectedGroupTag.value) return;
  await speedtestStore.testLatency(selectedGroupTag.value, [nodeTag]);
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
  if (!selectedGroupTag.value) return;
  await speedtestStore.startBatchTest(selectedGroupTag.value, currentNodes.value);
  toast.info("已启动批量串行测速任务");
}
</script>

<template>
  <div class="proxies-view">
    <header class="proxies-header">
      <div class="title-area">
        <h1>🌐 代理节点</h1>
        <p class="subtitle">管理出站节点分组，支持独立延迟测试与吞吐量测速</p>
      </div>

      <!-- 子功能切换 (节点列表 / 智能测速) -->
      <div class="sub-features-tab">
        <button
          class="tab-btn"
          :class="{ active: viewMode === 'nodes' }"
          @click="viewMode = 'nodes'"
        >
          🌐 节点大厅
        </button>
        <button
          class="tab-btn"
          :class="{ active: viewMode === 'speedtest' }"
          @click="viewMode = 'speedtest'"
        >
          📊 智能测速
        </button>
      </div>
    </header>

    <!-- 批量测速进度条 (全局浮层) -->
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

    <!-- 状态指示 -->
    <div v-if="loading" class="state-tip">
      ⏳ 正在获取代理节点...
    </div>

    <div v-else-if="error" class="state-tip error">
      ⚠️ {{ error }}
    </div>

    <div v-else-if="groups.length === 0" class="state-tip">
      <div class="placeholder-icon">🌐</div>
      <p>未检测到运行中的代理节点组。</p>
      <p class="sub-tip">请先在「设置」中导入订阅并启动 sing-box 核心。</p>
    </div>

    <div v-else class="proxies-container">
      <!-- 最近使用快捷切换 -->
      <div v-if="proxyStore.recentGroups.length > 1" class="recent-groups-bar">
        <span class="recent-label">⏱️ 最近常用:</span>
        <div class="recent-tags">
          <button
            v-for="tag in proxyStore.recentGroups"
            :key="tag"
            class="recent-tag"
            :class="{ active: tag === selectedGroupTag }"
            @click="handleGroupSelect(tag)"
          >
            {{ tag }}
          </button>
        </div>
      </div>

      <!-- 分组 Tabs 切换 -->
      <div class="group-bar">
        <div class="group-tabs">
          <button
            v-for="group in groups"
            :key="group.tag"
            class="group-tab"
            :class="{ active: group.tag === selectedGroupTag }"
            @click="handleGroupSelect(group.tag)"
          >
            <span class="group-name">{{ group.tag }}</span>
            <span v-if="group.now" class="group-now">({{ group.now }})</span>
          </button>
        </div>

        <div class="action-buttons">
          <button class="btn-action" @click="handleRunLatency">⚡ 测延迟</button>
          <button class="btn-action primary" @click="showConfirmModal = true">📶 批量测速</button>
          <button class="btn-action" @click="proxyStore.fetchGroups">🔄 刷新</button>
        </div>
      </div>

      <!-- 模式 1: 节点大厅 (Node Grid) -->
      <div v-if="viewMode === 'nodes'" class="node-grid">
        <NodeCard
          v-for="node in proxyStore.nodeMap.get(selectedGroupTag) ?? []"
          :key="node.tag"
          :node-tag="node.tag"
          :node-type="node.type"
          :is-active="node.is_active"
          :latency="speedtestStore.latencyMap[node.tag]"
          :speed-bps="speedtestStore.throughputMap[node.tag]?.download_bps"
          @select="handleNodeSelect(node.tag)"
          @test-latency="handleSingleLatency(node.tag)"
          @test-speed="handleSingleSpeed(node.tag)"
        />
      </div>

      <!-- 模式 2: 测速大厅看板 (Speedtest Dashboard) -->
      <div v-else class="speedtest-panel">
        <div class="speedtest-grid">
          <NodeCard
            v-for="nodeTag in currentNodes"
            :key="nodeTag"
            :node-tag="nodeTag"
            node-type="NODE"
            :is-active="nodeTag === (groups.find(g => g.tag === selectedGroupTag)?.now)"
            :latency="speedtestStore.latencyMap[nodeTag]"
            :speed-bps="speedtestStore.throughputMap[nodeTag]?.download_bps"
            @select="handleNodeSelect(nodeTag)"
            @test-latency="handleSingleLatency(nodeTag)"
            @test-speed="handleSingleSpeed(nodeTag)"
          />
        </div>
      </div>
    </div>

    <!-- 批量测速确认 Modal -->
    <Teleport to="body">
      <div v-if="showConfirmModal" class="modal-backdrop" @click.self="showConfirmModal = false">
        <div class="modal-card glass-effect">
          <h3>⚠️ 批量吞吐量测速确认</h3>
          <p>将对分组 <strong>「{{ selectedGroupTag }}」</strong> 的 <strong>{{ currentNodes.length }}</strong> 个节点依次进行带宽测试。</p>
          <div class="estimate-box">
            <div>⏱️ 预计耗时: 约 {{ Math.ceil(currentNodes.length * speedtestStore.THROUGHPUT_TEST_DURATION_SEC / 60) }} 分钟</div>
            <div>📉 预计流量消耗: 约 {{ Math.ceil(currentNodes.length * (speedtestStore.THROUGHPUT_TEST_CHUNK_BYTES / (1024 * 1024))) }} MB</div>
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
.proxies-view {
  padding: 48px 24px 24px 24px;
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.proxies-header {
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

.sub-features-tab {
  display: flex;
  gap: 6px;
  background: rgba(255, 255, 255, 0.04);
  padding: 4px;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.tab-btn {
  padding: 6px 14px;
  border-radius: 8px;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.tab-btn.active {
  background: rgba(0, 242, 254, 0.15);
  color: #fff;
  font-weight: 600;
}

.batch-progress-card {
  padding: 14px 18px;
  background: rgba(0, 242, 254, 0.08);
  border: 1px solid rgba(0, 242, 254, 0.25);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
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

.group-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.group-tabs {
  display: flex;
  gap: 8px;
  overflow-x: auto;
}

.group-tab {
  padding: 6px 14px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
  cursor: pointer;
  display: flex;
  gap: 6px;
  align-items: center;
}

.group-tab.active {
  background: rgba(0, 242, 254, 0.15);
  border-color: #00f2fe;
  color: #fff;
  font-weight: 600;
}

.action-buttons {
  display: flex;
  gap: 8px;
}

.btn-action {
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  color: #fff;
  font-size: 12px;
  cursor: pointer;
}

.btn-action.primary {
  background: rgba(0, 242, 254, 0.15);
  border-color: #00f2fe;
  color: #00f2fe;
  font-weight: 600;
}

.node-grid, .speedtest-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
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

.state-tip {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: rgba(255, 255, 255, 0.4);
}
.recent-groups-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.06);
  padding: 8px 14px;
  border-radius: 10px;
  margin-bottom: 12px;
}

.recent-label {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.4);
  font-weight: 600;
}

.recent-tags {
  display: flex;
  gap: 8px;
}

.recent-tag {
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.7);
  font-size: 11px;
  padding: 3px 8px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.recent-tag:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
  border-color: rgba(0, 242, 254, 0.2);
}

.recent-tag.active {
  background: rgba(0, 242, 254, 0.12);
  border-color: rgba(0, 242, 254, 0.4);
  color: #00f2fe;
}
</style>
