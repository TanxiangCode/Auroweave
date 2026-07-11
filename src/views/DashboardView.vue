<script setup lang="ts">
/**
 * Dashboard 首页 — 中央能量核 + 实时折线图 + 三张快捷卡片
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

import { DASHBOARD_CARD_COUNT } from "@/constants";

const router = useRouter();
const connectionStore = useConnectionStore();
const proxyStore = useProxyStore();

const { smoothDownloadSpeed, activeConnectionCount, totalDownload, totalUpload } = storeToRefs(connectionStore);
const { rotationDeg } = useFluidWave({ speedBps: smoothDownloadSpeed });

const cards = [
  { id: "proxies", icon: "proxies", label: "代理节点", desc: "节点大厅与切换", route: "/proxies" },
  { id: "routing", icon: "routing", label: "分流配置", desc: "应用级规则与矩阵", route: "/routing" },
  { id: "audit",   icon: "audit", label: "安全审计", desc: "实时抓包与 DNS 状态", route: "/audit" },
].slice(0, DASHBOARD_CARD_COUNT);

function toggleProxy() {
  const targetMode = proxyStore.proxyMode === "direct" ? "rule" : "direct";
  proxyStore.changeProxyMode(targetMode);
}

function changeMode(mode: "global" | "rule" | "direct") {
  proxyStore.changeProxyMode(mode);
}

onMounted(() => {
  proxyStore.fetchGroups();
});
</script>

<template>
  <div class="dashboard">
    <div class="energy-section">
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

    <!-- 中间控制与监控面板 (双栏布局) -->
    <div class="middle-panel">
      <!-- 左栏: 控制与快速统计 -->
      <div class="left-col">
        <!-- 代理模式切换与活动出站 -->
        <div class="control-card glass-effect">
          <div class="active-node-row">
            <span class="node-label">
              <SvgIcon name="routing" :size="14" style="margin-right: 6px;" />
              当前出站
            </span>
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
              {{ mode === "global" ? "全局" : mode === "rule" ? "规则" : "直连" }}
            </button>
          </div>
        </div>

        <!-- 快速统计 -->
        <div class="stats-overview">
          <div class="stat-pill">
            <span class="pill-label">活动连接</span>
            <span class="pill-val">{{ activeConnectionCount }} 条</span>
          </div>
          <div class="stat-pill">
            <span class="pill-label">下载</span>
            <span class="pill-val">{{ connectionStore.formatBytes(totalDownload) }}</span>
          </div>
          <div class="stat-pill">
            <span class="pill-label">上传</span>
            <span class="pill-val">{{ connectionStore.formatBytes(totalUpload) }}</span>
          </div>
        </div>
      </div>

      <!-- 右栏: 实时网速折线图 (大气展现) -->
      <div class="right-col chart-container">
        <SpeedChart />
      </div>
    </div>

    <!-- 极客精致快捷胶囊入口 -->
    <div class="quick-links">
      <button
        v-for="card in cards"
        :key="card.id"
        class="quick-link-btn"
        @click="router.push(card.route)"
      >
        <SvgIcon :name="card.icon" :size="16" class="link-icon" />
        <span class="link-label">{{ card.label }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 24px;
  padding: 24px;
  overflow-y: auto;
}

.energy-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-bottom: 8px;
}

/* ---- 能量核 ---- */
.energy-core {
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: transform var(--duration-fast) var(--ease-out);
}

.energy-core:hover {
  transform: scale(1.03);
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
  box-shadow: 0 0 20px rgba(0, 242, 254, 0.4), var(--shadow-glow-cyan);
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

/* 常用策略组已移除 */

/* ---- 中间面板 ---- */
.middle-panel {
  display: grid;
  grid-template-columns: 320px 1fr;
  gap: 24px;
  width: 100%;
}

.left-col, .right-col {
  display: flex;
  flex-direction: column;
  gap: 16px;
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

/* ---- 控制面板 ---- */
.control-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.active-node-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-bottom: 4px;
}

.node-label {
  display: flex;
  align-items: center;
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.node-value {
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  color: var(--accent-cyan);
  background: var(--accent-cyan-glow);
  padding: 2px 10px;
  border-radius: var(--radius-sm);
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mode-selector {
  display: flex;
  background: var(--layer-2);
  padding: 2px;
  border-radius: var(--radius-sm);
  gap: 2px;
}

.mode-btn {
  flex: 1;
  padding: 6px 0;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  border-radius: var(--radius-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.mode-btn.active {
  background: var(--layer-1);
  color: var(--accent-blue);
  font-weight: var(--weight-bold);
  box-shadow: var(--shadow-sm);
}

/* ---- 快速统计 ---- */
.stats-overview {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 12px;
}

.stat-pill {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 12px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  transition: all var(--duration-fast) var(--ease-out);
}

.stat-pill:hover {
  background: var(--layer-2);
  border-color: var(--border-accent);
  transform: translateY(-2px);
}

.pill-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  font-weight: var(--weight-medium);
}

.pill-val {
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
}

/* ---- 图表面板 ---- */
.chart-container {
  display: flex;
  flex-direction: column;
  width: 100%;
}

/* ---- 极客快捷入口胶囊化 ---- */
.quick-links {
  display: flex;
  justify-content: center;
  gap: 20px;
  margin-top: 12px;
}

.quick-link-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 24px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  cursor: pointer;
  transition: all var(--duration-normal) var(--ease-out);
}

.quick-link-btn:hover {
  background: var(--layer-2);
  border-color: var(--accent-blue);
  box-shadow: var(--shadow-glow-blue);
  transform: translateY(-1px);
}

.link-icon {
  color: var(--text-secondary);
  transition: color var(--duration-fast);
}

.quick-link-btn:hover .link-icon {
  color: var(--accent-blue);
}

.link-label {
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
}
</style>
