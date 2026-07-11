<script setup lang="ts">
/**
 * 代理节点视图 (重构：双栏行列表布局，统一数据源)
 * 作者: TanXiang
 */
import { onMounted, ref, computed } from "vue";
import { useRoute } from "vue-router";
import { useProxyStore } from "@/stores/proxy.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useToast } from "@/composables/useToast";
import { storeToRefs } from "pinia";
import NodeCard from "@/components/proxy/NodeCard.vue";
import SvgIcon from "@/components/common/SvgIcon.vue";

const proxyStore = useProxyStore();
const speedtestStore = useSpeedtestStore();
const toast = useToast();
const route = useRoute();

const { groups, loading } = storeToRefs(proxyStore);
const selectedGroupTag = ref<string>("");
const showConfirmModal = ref(false);

onMounted(async () => {
  await proxyStore.fetchGroups();
  await speedtestStore.init();
  if (groups.value.length > 0) {
    const qGroup = route.query.group as string;
    const exists = groups.value.some((g) => g.tag === qGroup);
    selectedGroupTag.value = exists ? qGroup : groups.value[0].tag;
    await proxyStore.fetchGroupNodes(selectedGroupTag.value);
  }
});

const currentNodes = computed(() => {
  const g = groups.value.find((x) => x.tag === selectedGroupTag.value);
  return g ? g.proxies : [];
});

const currentGroup = computed(() => {
  return groups.value.find((x) => x.tag === selectedGroupTag.value);
});

const isSelectorGroup = computed(() => {
  return currentGroup.value?.type === "selector";
});

// 计算当前处于激活出口链路上的所有策略组 tag
const routingGroupTags = computed(() => {
  const tags = new Set<string>();
  const primary = groups.value.find((g) => g.type === "selector");
  if (!primary) return tags;

  tags.add(primary.tag);

  let currentTagName = primary.now;
  // 限制循环次数防死循环，最多 10 层
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

async function handleGroupSelect(groupTag: string) {
  selectedGroupTag.value = groupTag;
  await proxyStore.fetchGroupNodes(groupTag);

  // 记录使用计数
  proxyStore.recordGroupUsage(groupTag);
}

async function handleNodeSelect(nodeTag: string) {
  if (!selectedGroupTag.value) return;
  if (!isSelectorGroup.value) {
    toast.warning("不支持切换", "该策略组为自动或非手动选择类型，无法手动指定节点。");
    return;
  }
  console.log('handleNodeSelect')
  const res = await proxyStore.selectNode(selectedGroupTag.value, nodeTag);
  if (res && res.success) {
    toast.success("节点已切换", `当前出站: ${nodeTag}`);
  } else {
    toast.error("切换节点失败", res?.error || "未知错误");
  }
}

async function handleRunLatency() {
  if (!selectedGroupTag.value) return;
  toast.info("正在并发测试延迟...");
  const nodes = proxyStore.nodeMap.get(selectedGroupTag.value) ?? [];
  // 过滤：排除 selector、urltest 等策略组类型的子项，仅对具体的真实代理服务器进行延迟测试
  const tags = nodes
    .filter((n) => !["selector", "urltest", "fallback"].includes(n.type.toLowerCase()))
    .map((n) => n.tag);

  if (tags.length === 0) {
    toast.warning("该策略组内没有可供测试的真实节点");
    return;
  }

  await speedtestStore.testLatency(selectedGroupTag.value, tags);
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

    <!-- 双栏布局区域 -->
    <div v-if="groups.length > 0" class="proxies-layout">
      <!-- 左栏: 分组选择器 -->
      <aside class="sidebar-groups glass-effect">
        <!-- 最近常用快捷标签 -->
        <div v-if="proxyStore.recentGroups.length > 1" class="recent-section">
          <span class="section-title">
            <SvgIcon name="clock" :size="12" style="margin-right: 4px;" />
            最近常用
          </span>
          <div class="recent-list">
            <button
              v-for="tag in proxyStore.recentGroups"
              :key="tag"
              class="recent-item"
              :class="{ active: tag === selectedGroupTag }"
              @click="handleGroupSelect(tag)"
            >
              {{ tag }}
            </button>
          </div>
        </div>

        <span class="section-title">
          <SvgIcon name="folder" :size="12" style="margin-right: 4px;" />
          策略组
        </span>
        <div class="groups-list">
          <button
            v-for="group in groups"
            :key="group.tag"
            class="group-item"
            :class="{ 
              active: group.tag === selectedGroupTag,
              'in-route': routingGroupTags.has(group.tag)
            }"
            @click="handleGroupSelect(group.tag)"
          >
            <div class="group-header-info">
              <div class="group-name-wrapper">
                <span v-if="routingGroupTags.has(group.tag)" class="route-dot" title="当前活跃出口链路成员"></span>
                <span class="group-name">{{ group.tag }}</span>
              </div>
              <span class="group-badge">{{ group.type }}</span>
            </div>
            <span 
              v-if="group.now" 
              class="group-current-node"
              :class="{ 'highlight-now': routingGroupTags.has(group.tag) }"
            >
              {{ group.now }}
            </span>
          </button>
        </div>
      </aside>

      <!-- 右栏: 节点行列表 -->
      <main class="nodes-content glass-effect">
        <div class="nodes-header">
          <div class="header-left">
            <h2>{{ selectedGroupTag }}</h2>
            <span class="nodes-count" v-if="proxyStore.nodeMap.get(selectedGroupTag)">
              共 {{ (proxyStore.nodeMap.get(selectedGroupTag) || []).length }} 个节点
            </span>
          </div>
          <div class="action-buttons">
            <button class="btn-action" @click="handleRunLatency">
              <SvgIcon name="bolt" :size="12" style="margin-right: 4px;" />
              测延迟
            </button>
            <button class="btn-action primary" @click="showConfirmModal = true">
              <SvgIcon name="wifi" :size="12" style="margin-right: 4px;" />
              批量测速
            </button>
            <button class="btn-action" @click="proxyStore.fetchGroups">
              <SvgIcon name="refresh" :size="12" style="margin-right: 4px;" />
              刷新
            </button>
          </div>
        </div>

        <div v-if="loading" class="state-tip">
          ⏳ 正在加载节点列表...
        </div>
        <div v-else-if="!proxyStore.nodeMap.get(selectedGroupTag) || (proxyStore.nodeMap.get(selectedGroupTag) || []).length === 0" class="state-tip">
          📭 暂无节点数据，请点击刷新
        </div>
        <div v-else class="nodes-scroll">
          <div class="nodes-list">
            <NodeCard
              v-for="node in proxyStore.nodeMap.get(selectedGroupTag) ?? []"
              :key="node.tag"
              :node-tag="node.tag"
              :node-type="node.type"
              :is-active="node.is_active"
              :latency="speedtestStore.latencyMap[node.tag]"
              :speed-bps="speedtestStore.throughputMap[node.tag]?.download_bps"
              :is-selectable="isSelectorGroup"
              @select="handleNodeSelect(node.tag)"
              @test-latency="handleSingleLatency(node.tag)"
              @test-speed="handleSingleSpeed(node.tag)"
            />
          </div>
        </div>
      </main>
    </div>

    <!-- 异常或空状态 -->
    <div v-else class="state-tip-full">
      <SvgIcon name="proxies" :size="48" style="color: var(--text-tertiary);" />
      <p>未检测到运行中的代理节点组。</p>
      <p class="sub-tip">请确保 Sing-box 后台核心已成功启动且配置导入正确。</p>
      <button class="btn-retry" @click="proxyStore.fetchGroups">
        <SvgIcon name="refresh" :size="12" style="margin-right: 4px;" />
        重试刷新
      </button>
    </div>

    <!-- 批量测速确认 Modal -->
    <Teleport to="body">
      <div v-if="showConfirmModal" class="modal-backdrop" @click.self="showConfirmModal = false">
        <div class="modal-card glass-effect">
          <h3>⚠️ 批量吞吐量测速确认</h3>
          <p>将对分组 <strong>「{{ selectedGroupTag }}」</strong> 的所有节点依次进行带宽测试。</p>
          <div class="estimate-box">
            <div>⏱️ 预计总耗时: 约 {{ Math.ceil((proxyStore.nodeMap.get(selectedGroupTag) || []).length * speedtestStore.THROUGHPUT_TEST_DURATION_SEC / 60) }} 分钟</div>
            <div>📉 预计流量消耗: 约 {{ Math.ceil((proxyStore.nodeMap.get(selectedGroupTag) || []).length * (speedtestStore.THROUGHPUT_TEST_CHUNK_BYTES / (1024 * 1024))) }} MB</div>
          </div>
          <p class="warning-tip">测速将以串行队列形式进行，以获得最准确的无干扰结果。</p>
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
  padding: 20px;
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.proxies-layout {
  display: flex;
  gap: 16px;
  height: 100%;
  overflow: hidden;
}

.glass-effect {
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
}

/* ---- 左栏: 分组列表 ---- */
.sidebar-groups {
  width: 220px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px;
  overflow-y: auto;
  flex-shrink: 0;
}

.section-title {
  display: flex;
  align-items: center;
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  color: var(--text-tertiary);
  text-transform: uppercase;
  margin-top: 8px;
}

.recent-list, .groups-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.recent-item, .group-item {
  display: flex;
  flex-direction: column;
  padding: 8px 12px;
  border-radius: var(--radius-md);
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  text-align: left;
  transition: all var(--duration-fast) var(--ease-out);
}

.recent-item:hover, .group-item:hover {
  background: var(--border-subtle);
  color: var(--text-primary);
}

.recent-item.active, .group-item.active {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
  font-weight: var(--weight-semibold);
}

.group-header-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

.group-name-wrapper {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex: 1;
}

.route-dot {
  width: 6px;
  height: 6px;
  background-color: var(--accent-green);
  border-radius: 50%;
  box-shadow: 0 0 8px var(--accent-green);
  flex-shrink: 0;
  animation: pulse-green 2s infinite;
}

@keyframes pulse-green {
  0% {
    box-shadow: 0 0 0 0 rgba(52, 211, 153, 0.7);
  }
  70% {
    box-shadow: 0 0 0 6px rgba(52, 211, 153, 0);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(52, 211, 153, 0);
  }
}

.group-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-badge {
  font-size: var(--text-xs);
  padding: 1px 4px;
  background: var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.group-current-node {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-top: 2px;
  transition: color var(--duration-fast);
}

.group-current-node.highlight-now {
  color: var(--accent-cyan);
  font-weight: var(--weight-medium);
}

/* ---- 右栏: 节点内容 ---- */
.nodes-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 20px;
  overflow: hidden;
}

.nodes-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
  border-bottom: 1px solid var(--border-subtle);
  padding-bottom: 12px;
}

.header-left h2 {
  font-size: var(--text-md);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.nodes-count {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.action-buttons {
  display: flex;
  gap: 8px;
}

.btn-action {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-action:hover {
  background: var(--border-strong);
  border-color: var(--border-accent);
}

.btn-action.primary {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
  font-weight: var(--weight-semibold);
}

.btn-action.primary:hover {
  background: var(--accent-blue);
  color: var(--text-on-accent);
}

.nodes-scroll {
  flex: 1;
  overflow-y: auto;
  margin-top: 12px;
  padding-right: 4px;
}

.nodes-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* ---- 批量测速进度条 ---- */
.batch-progress-card {
  padding: 12px 16px;
  margin-bottom: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.progress-info {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-xs);
}

.progress-bar-bg {
  height: 6px;
  background: var(--layer-2);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: var(--accent-cyan);
  box-shadow: var(--shadow-glow-cyan);
  transition: width var(--duration-normal) var(--ease-out);
}

.btn-cancel {
  align-self: flex-end;
  background: transparent;
  border: none;
  color: var(--accent-red);
  font-size: var(--text-xs);
  cursor: pointer;
}

/* ---- 异常与空状态 ---- */
.state-tip-full {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
  gap: 12px;
}

.placeholder-icon {
  font-size: var(--text-2xl);
}

.sub-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.btn-retry {
  display: flex;
  align-items: center;
  padding: 8px 16px;
  background: var(--accent-blue-glow);
  border: 1px solid var(--accent-blue);
  border-radius: var(--radius-sm);
  color: var(--accent-blue);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-retry:hover {
  background: var(--accent-blue);
  color: var(--text-on-accent);
}

.state-tip {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

/* ---- 测速确认 Modal ---- */
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 99999;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: var(--blur-panel);
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-card {
  width: 400px;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.estimate-box {
  background: var(--layer-2);
  padding: 12px;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.warning-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

.btn {
  padding: 6px 16px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-normal);
  background: transparent;
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn.primary {
  background: var(--accent-blue);
  color: var(--text-on-accent);
  border-color: var(--accent-blue);
}

.btn.primary:hover {
  opacity: 0.9;
}

.btn.text {
  border-color: transparent;
  color: var(--text-secondary);
}

.btn.text:hover {
  background: var(--border-subtle);
  color: var(--text-primary);
}
</style>
