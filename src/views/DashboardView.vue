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

function selectRecentGroup(tag: string) {
  router.push({ path: "/proxies", query: { group: tag } });
}

function getGroupNow(tag: string) {
  const g = proxyStore.groups.find((x) => x.tag === tag);
  return g ? g.now : "";
}

onMounted(() => {
  proxyStore.fetchGroups();
});
</script>

<template>
  <div class="dashboard">
    <!-- 中央动态能量核 (Fluid Wave 驱动) -->
    <div class="energy-section">
      <div class="energy-core" :class="{ connected: proxyStore.proxyMode !== 'direct' }">
        <div
          class="energy-ring"
          :style="{ transform: `rotate(${rotationDeg}deg)` }"
        >
          <div class="energy-inner">
            <span class="energy-status">
              <span class="status-dot-indicator" :class="{ active: proxyStore.proxyMode !== 'direct' }"></span>
              {{ proxyStore.proxyMode !== "direct" ? "已开启代理" : "已关闭 (直连)" }}
            </span>
            <span class="energy-speed">
              ⚡ {{ connectionStore.formatSpeed(smoothDownloadSpeed) }}
            </span>
            <span class="energy-node">
              模式: {{ proxyStore.proxyMode.toUpperCase() }}
            </span>
          </div>
        </div>
      </div>

      <!-- 最近使用的策略组快捷切换 (一键直达) -->
      <div v-if="proxyStore.recentGroups.length > 0" class="recent-groups-row">
        <span class="recent-label">
          <SvgIcon name="clock" :size="12" style="margin-right: 4px;" />
          常用策略组:
        </span>
        <div class="recent-capsules">
          <button
            v-for="tag in proxyStore.recentGroups.slice(0, 4)"
            :key="tag"
            class="recent-capsule"
            @click="selectRecentGroup(tag)"
            :title="`点击管理 ${tag}`"
          >
            <span class="group-tag">{{ tag }}</span>
            <span class="group-now" v-if="getGroupNow(tag)">: {{ getGroupNow(tag) }}</span>
          </button>
        </div>
      </div>
    </div>

    <!-- 中间控制与监控面板 (双栏布局) -->
    <div class="middle-panel">
      <!-- 左栏: 控制与快速统计 -->
      <div class="left-col">
        <!-- 代理开关及模式切换 -->
        <div class="control-card glass-effect">
          <div class="control-header">
            <span class="control-title">
              <SvgIcon name="power" :size="14" style="margin-right: 4px;" />
              代理控制
            </span>
            <button
              class="power-btn"
              :class="{ active: proxyStore.proxyMode !== 'direct' }"
              @click="toggleProxy"
              :title="proxyStore.proxyMode !== 'direct' ? '点击关闭代理' : '点击开启代理'"
            >
              {{ proxyStore.proxyMode !== "direct" ? "已开启" : "已关闭" }}
            </button>
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

      <!-- 右栏: 瘦身后的实时网速折线图 -->
      <div class="right-col glass-effect chart-container">
        <SpeedChart :compact="true" />
      </div>
    </div>

    <!-- 三张启动卡片 -->
    <div class="launch-cards">
      <button
        v-for="card in cards"
        :key="card.id"
        class="launch-card"
        @click="router.push(card.route)"
      >
        <SvgIcon :name="card.icon" :size="26" class="card-icon" />
        <span class="card-label">{{ card.label }}</span>
        <span class="card-desc">{{ card.desc }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 20px;
  padding: 24px;
  overflow-y: auto;
}

.energy-section {
  display: flex;
  flex-direction: column;
  align-items: center;
}

/* ---- 能量核 ---- */
.energy-core {
  display: flex;
  align-items: center;
  justify-content: center;
}

.energy-ring {
  width: 180px;
  height: 180px;
  border-radius: 50%;
  padding: 4px;
  background: var(--energy-active);
  box-shadow: var(--shadow-glow-cyan);
  transition: all var(--duration-normal) var(--ease-out);
}

.energy-core:not(.connected) .energy-ring {
  background: var(--energy-idle);
  box-shadow: none;
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
  gap: 6px;
  padding: 12px;
}

.energy-status {
  display: flex;
  align-items: center;
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--weight-medium);
}

.status-dot-indicator {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  margin-right: 6px;
  background: var(--text-tertiary);
  transition: all var(--duration-fast);
}

.status-dot-indicator.active {
  background: var(--accent-green);
  box-shadow: var(--shadow-glow-green);
}

.energy-speed {
  font-size: var(--text-lg);
  color: var(--accent-cyan);
  font-weight: var(--weight-bold);
  text-shadow: var(--shadow-glow-cyan);
}

.energy-node {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.recent-groups-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 14px;
  background: var(--layer-1);
  border: 1px solid var(--border-subtle);
  padding: 6px 14px;
  border-radius: var(--radius-full);
}

.recent-label {
  display: flex;
  align-items: center;
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  font-weight: var(--weight-semibold);
}

.recent-capsules {
  display: flex;
  gap: 8px;
}

.recent-capsule {
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  padding: 2px 8px;
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.recent-capsule:hover {
  border-color: var(--border-accent);
  color: var(--text-primary);
}

.group-tag {
  font-weight: var(--weight-bold);
}

.group-now {
  font-family: var(--font-sans);
}

/* ---- 中间面板 ---- */
.middle-panel {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  width: 100%;
}

.left-col, .right-col {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.glass-effect {
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  padding: 16px;
}

/* ---- 控制面板 ---- */
.control-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.control-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.control-title {
  display: flex;
  align-items: center;
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.power-btn {
  padding: 4px 12px;
  border-radius: var(--radius-full);
  border: 1px solid var(--border-strong);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.power-btn.active {
  background: var(--accent-green-glow);
  border-color: var(--accent-green);
  color: var(--accent-green);
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
  gap: 8px;
}

.stat-pill {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 8px;
  background: var(--layer-1);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.pill-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.pill-val {
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-weight: var(--weight-semibold);
}

/* ---- 图表面板 ---- */
.chart-container {
  display: flex;
  align-items: center;
  justify-content: center;
}

/* ---- 启动卡片 ---- */
.launch-cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

.launch-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 16px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  color: var(--text-primary);
  cursor: pointer;
  transition: all var(--duration-normal) var(--ease-out);
}

.launch-card:hover {
  background: var(--layer-2);
  border-color: var(--border-accent);
  transform: translateY(-2px);
}

.card-icon {
  color: var(--text-secondary);
}

.launch-card:hover .card-icon {
  color: var(--accent-blue);
}

.card-label {
  font-size: var(--text-sm);
  font-weight: var(--weight-bold);
}

.card-desc {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-align: center;
}
</style>
