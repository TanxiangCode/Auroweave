<script setup lang="ts">
/**
 * Dashboard 首页 — 环绕双翼镜像对称布局
 * 作者: TanXiang
 */
import { onMounted } from "vue";
import { useConnectionStore } from "@/stores/connection.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useFluidWave } from "@/composables/useFluidWave";
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";
import SpeedChart from "@/components/charts/SpeedChart.vue";
import SvgIcon from "@/components/common/SvgIcon.vue";

const router = useRouter();
const connectionStore = useConnectionStore();
const proxyStore = useProxyStore();

const {
  rawDownloadSpeed,
  rawUploadSpeed,
  smoothDownloadSpeed,
  activeConnectionCount,
  totalDownload,
  totalUpload
} = storeToRefs(connectionStore);

const { rotationDeg } = useFluidWave({ speedBps: smoothDownloadSpeed });

function toggleProxy() {
  const targetMode = proxyStore.proxyMode === "direct" ? "rule" : "direct";
  proxyStore.changeProxyMode(targetMode);
}

function changeMode(mode: "global" | "rule" | "direct") {
  proxyStore.changeProxyMode(mode);
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

onMounted(() => {
  proxyStore.fetchGroups();
});
</script>

<template>
  <div class="dashboard-layout">
    <!-- 上部：“两翼对称”三栏大格局 -->
    <div class="top-panel-row">
      <!-- 左翼：出站控制 -->
      <div class="control-wing glass-effect">
        <div class="wing-header">
          <SvgIcon name="settings" :size="13" style="color: var(--accent-blue);" />
          <span class="wing-title">控制面板</span>
        </div>
        <div class="control-card">
          <div class="active-node-row">
            <span class="node-label">当前出站</span>
            <span class="node-value" :title="proxyStore.workingNodeName">
              {{ proxyStore.workingNodeName }}
            </span>
          </div>
          <div class="mode-selector">
            <button
              v-for="mode in ['global', 'rule', 'direct']"
              :key="mode"
              class="mode-btn"
              :class="{ active: proxyStore.proxyMode === mode }"
              @click="changeMode(mode as any)"
            >
              {{ mode === 'global' ? '全局' : mode === 'rule' ? '规则' : '直连' }}
            </button>
          </div>
        </div>
      </div>

      <!-- 中央：旋转圆环能量核 (视觉绝对重心) -->
      <div class="energy-wing">
        <div
          class="energy-core"
          :class="{ connected: proxyStore.proxyMode !== 'direct' }"
          @click="toggleProxy"
          :title="proxyStore.proxyMode !== 'direct' ? '点击关闭代理' : '点击开启代理'"
        >
          <div
            class="energy-ring"
            :style="{ transform: 'rotate(' + rotationDeg + 'deg)' }"
          >
            <div class="energy-inner">
              <template v-if="proxyStore.proxyMode !== 'direct'">
                <span class="energy-status-text">CONNECTED</span>
                <span class="energy-mode-tag">{{ proxyStore.proxyMode.toUpperCase() }}</span>
                <span class="energy-info-text">{{ activeConnectionCount }} 个连接</span>
              </template>
              <template v-else>
                <span class="energy-status-text idle">TAP TO CONNECT</span>
                <span class="status-dot-indicator"></span>
                <span class="energy-info-text idle">已关闭 (直连)</span>
              </template>
            </div>
          </div>
        </div>
      </div>

      <!-- 右翼：流量统计小卡片 (垂直层叠镜像) -->
      <div class="stats-wing glass-effect">
        <div class="wing-header">
          <SvgIcon name="audit" :size="13" style="color: var(--accent-cyan);" />
          <span class="wing-title">实时大盘</span>
        </div>
        <div class="stats-col">
          <!-- 连接数 -> 跳转审计 -->
          <div
            class="stat-pill clickable-pill"
            @click="router.push('/audit')"
            title="点击查看实时连接审计"
          >
            <span class="pill-label">活动连接</span>
            <span class="pill-val cyan-glow">{{ activeConnectionCount }} 条</span>
          </div>
          <!-- 下载 -> 跳转统计 -->
          <div
            class="stat-pill clickable-pill"
            @click="router.push('/stats')"
            title="点击查看详细流量统计"
          >
            <span class="pill-label">累计下载</span>
            <span class="pill-val">{{ formatBytes(totalDownload) }}</span>
          </div>
          <!-- 上传 -> 跳转统计 -->
          <div
            class="stat-pill clickable-pill"
            @click="router.push('/stats')"
            title="点击查看详细流量统计"
          >
            <span class="pill-label">累计上传</span>
            <span class="pill-val">{{ formatBytes(totalUpload) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 下部：实时折线图底托 -->
    <div class="bottom-panel-row">
      <div class="chart-container glass-effect">
        <div class="chart-header">
          <span class="chart-title">
            <span class="status-dot-pulse"></span>
            实时网络流量趋势
          </span>
          <div class="chart-speed-legend">
            <span class="speed-down">
              ↓ 下载 {{ connectionStore.formatSpeed(rawDownloadSpeed) }}
            </span>
            <span class="speed-up">
              ↑ 上传 {{ connectionStore.formatSpeed(rawUploadSpeed) }}
            </span>
          </div>
        </div>
        <SpeedChart />
      </div>
    </div>
  </div>
</template>

<style scoped>
.dashboard-layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 20px;
  padding: 24px;
  overflow: hidden; /* 保证在 1020x680 分辨率下完美贴合且不出现滚动条 */
}

/* ---- 上部：“两翼对称”三栏 ---- */
.top-panel-row {
  display: grid;
  grid-template-columns: 320px 1fr 260px;
  gap: 24px;
  align-items: center;
  width: 100%;
}

.wing-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 16px;
  border-bottom: 1px solid var(--border-subtle);
  padding-bottom: 10px;
}

.wing-title {
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 1px;
}

.control-wing {
  height: 260px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.control-card {
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;
  justify-content: center;
}

.active-node-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.node-label {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.node-value {
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  color: var(--accent-cyan);
  background: var(--accent-cyan-glow);
  padding: 4px 12px;
  border-radius: var(--radius-sm);
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-shadow: 0 0 8px var(--accent-cyan-glow);
}

.mode-selector {
  display: flex;
  background: var(--layer-2);
  padding: 3px;
  border-radius: var(--radius-sm);
  gap: 4px;
}

.mode-btn {
  flex: 1;
  padding: 8px 0;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  border-radius: var(--radius-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.mode-btn:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.02);
}

.mode-btn.active {
  background: var(--layer-1);
  color: var(--accent-blue);
  font-weight: var(--weight-bold);
  box-shadow: var(--shadow-sm);
}

/* ---- 中央：圆环能量核 ---- */
.energy-wing {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 260px;
}

.energy-core {
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: transform var(--duration-fast) var(--ease-out);
}

.energy-core:hover {
  transform: scale(1.02);
}

.energy-ring {
  width: 240px;
  height: 240px;
  border-radius: 50%;
  padding: 4px;
  background: var(--energy-active);
  box-shadow: var(--shadow-glow-cyan);
  transition: all var(--duration-normal) var(--ease-out);
}

.energy-core:hover .energy-ring {
  box-shadow: 0 0 24px rgba(0, 242, 254, 0.45), var(--shadow-glow-cyan);
}

.energy-core:not(.connected) .energy-ring {
  background: var(--energy-idle);
  box-shadow: none;
}

.energy-core:not(.connected):hover .energy-ring {
  background: var(--border-strong);
  box-shadow: 0 0 12px rgba(255, 255, 255, 0.1);
}

.energy-inner {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: var(--layer-0);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 18px;
}

.energy-status-text {
  font-size: var(--text-sm);
  font-weight: var(--weight-bold);
  color: var(--accent-cyan);
  letter-spacing: 2px;
  text-shadow: var(--shadow-glow-cyan);
}

.energy-status-text.idle {
  color: var(--text-secondary);
  text-shadow: none;
}

.energy-mode-tag {
  background: var(--accent-blue-glow);
  color: var(--accent-blue);
  padding: 2px 10px;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
}

.energy-info-text {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.energy-info-text.idle {
  color: var(--text-tertiary);
}

.status-dot-indicator {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-tertiary);
}

/* ---- 右翼：流量大盘 ---- */
.stats-wing {
  height: 260px;
  display: flex;
  flex-direction: column;
}

.stats-col {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  justify-content: center;
}

.stat-pill {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  background: var(--layer-1);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  transition: all var(--duration-fast) var(--ease-out);
}

.stat-pill.clickable-pill {
  cursor: pointer;
}

.stat-pill.clickable-pill:hover {
  background: var(--layer-2);
  border-color: var(--accent-blue);
  box-shadow: 0 0 10px var(--accent-blue-glow);
  transform: translateX(-2px);
}

.pill-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--weight-semibold);
}

.pill-val {
  font-size: var(--text-xs);
  color: var(--text-primary);
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
}

.pill-val.cyan-glow {
  color: var(--accent-cyan);
  text-shadow: 0 0 4px var(--accent-cyan-glow);
}

/* ---- 下部：折线图底托 ---- */
.bottom-panel-row {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0; /* 允许折线图内部自适应伸缩而不溢出 */
}

.chart-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  padding: 18px 24px;
}

.chart-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.chart-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--text-sm);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.status-dot-pulse {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--accent-cyan);
  box-shadow: 0 0 8px var(--accent-cyan);
  animation: pulse 2s infinite;
}

.chart-speed-legend {
  display: flex;
  gap: 16px;
  font-size: var(--text-xs);
  font-family: var(--font-mono, monospace);
  font-weight: var(--weight-bold);
}

.speed-down {
  color: var(--accent-cyan);
}

.speed-up {
  color: var(--accent-purple, #b388ff);
}

.glass-effect {
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  padding: 20px;
  transition: border-color var(--duration-fast), box-shadow var(--duration-fast);
}

.glass-effect:hover {
  border-color: var(--border-accent);
}

@keyframes pulse {
  0% { transform: scale(0.9); opacity: 0.6; }
  50% { transform: scale(1.15); opacity: 1; }
  100% { transform: scale(0.9); opacity: 0.6; }
}
</style>
