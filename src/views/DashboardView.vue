<script setup lang="ts">
/**
 * Dashboard 首页 — 中央能量核 + 实时折线图 + 三张快捷卡片
 * 作者: TanXiang
 */
import { useConnectionStore } from "@/stores/connection.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useFluidWave } from "@/composables/useFluidWave";
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";
import SpeedChart from "@/components/charts/SpeedChart.vue";

import { DASHBOARD_CARD_COUNT } from "@/constants";

const router = useRouter();
const connectionStore = useConnectionStore();
const proxyStore = useProxyStore();

const { smoothDownloadSpeed, activeConnectionCount, totalDownload, totalUpload } = storeToRefs(connectionStore);
const { rotationDeg } = useFluidWave({ speedBps: smoothDownloadSpeed });

const cards = [
  { id: "proxies", icon: "🚀", label: "代理节点", desc: "节点大厅与切换", route: "/proxies" },
  { id: "routing", icon: "🛠️", label: "分流配置", desc: "应用级规则与矩阵", route: "/routing" },
  { id: "audit",   icon: "🔍", label: "安全审计", desc: "实时抓包与 DNS 状态", route: "/audit" },
].slice(0, DASHBOARD_CARD_COUNT);
</script>

<template>
  <div class="dashboard">
    <!-- 中央动态能量核 (Fluid Wave 驱动) -->
    <div class="energy-section">
      <div class="energy-core" :class="{ connected: connectionStore.isConnected }">
        <div
          class="energy-ring"
          :style="{ transform: `rotate(${rotationDeg}deg)` }"
        >
          <div class="energy-inner">
            <span class="energy-status">
              {{ connectionStore.isConnected ? "🟢 运行中" : "⚫ 未重连" }}
            </span>
            <span class="energy-speed">
              ⚡ {{ connectionStore.formatSpeed(smoothDownloadSpeed) }}
            </span>
            <span class="energy-node">
              当前模式: {{ proxyStore.proxyMode.toUpperCase() }}
            </span>
          </div>
        </div>
      </div>

      <!-- 快速统计面板 -->
      <div class="stats-overview">
        <div class="stat-pill">
          <span class="pill-label">活动连接</span>
          <span class="pill-val">{{ activeConnectionCount }} 条</span>
        </div>
        <div class="stat-pill">
          <span class="pill-label">累计下载</span>
          <span class="pill-val">{{ connectionStore.formatBytes(totalDownload) }}</span>
        </div>
        <div class="stat-pill">
          <span class="pill-label">累计上传</span>
          <span class="pill-val">{{ connectionStore.formatBytes(totalUpload) }}</span>
        </div>
      </div>
    </div>

    <!-- 实时网速折线图 -->
    <div class="chart-section">
      <SpeedChart />
    </div>

    <!-- 三张启动卡片 -->
    <div class="launch-cards">
      <button
        v-for="card in cards"
        :key="card.id"
        class="launch-card"
        @click="router.push(card.route)"
      >
        <span class="card-icon">{{ card.icon }}</span>
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
  gap: 16px;
}

/* ---- 能量核 ---- */
.energy-core {
  display: flex;
  align-items: center;
  justify-content: center;
}

.energy-ring {
  width: 170px;
  height: 170px;
  border-radius: 50%;
  padding: 4px;
  background: conic-gradient(
    from 0deg,
    #00f2fe,
    #4facfe,
    #a855f7,
    #00f2fe
  );
  box-shadow: 0 0 32px rgba(0, 242, 254, 0.35);
  transition: box-shadow 0.3s ease;
}

.energy-core:not(.connected) .energy-ring {
  background: conic-gradient(from 0deg, #2d2d2d, #3f3f46, #2d2d2d);
  box-shadow: none;
}

.energy-inner {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  background: rgba(18, 22, 34, 0.95);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 12px;
}

.energy-status {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.7);
  font-weight: 500;
}

.energy-speed {
  font-size: 18px;
  color: #00f2fe;
  font-weight: 700;
  text-shadow: 0 0 10px rgba(0, 242, 254, 0.4);
}

.energy-node {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
}

.stats-overview {
  display: flex;
  gap: 16px;
}

.stat-pill {
  display: flex;
  gap: 8px;
  align-items: center;
  padding: 6px 14px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 20px;
  font-size: 12px;
}

.pill-label {
  color: rgba(255, 255, 255, 0.5);
}

.pill-val {
  color: rgba(255, 255, 255, 0.9);
  font-weight: 600;
}

.chart-section {
  width: 100%;
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
  background: rgba(255, 255, 255, 0.03);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-lg, 14px);
  color: var(--text-primary);
  cursor: pointer;
  transition: all 0.2s ease;
}

.launch-card:hover {
  background: rgba(0, 242, 254, 0.08);
  border-color: rgba(0, 242, 254, 0.3);
  transform: translateY(-2px);
}

.card-icon {
  font-size: 26px;
  line-height: 1;
}

.card-label {
  font-size: 14px;
  font-weight: 600;
}

.card-desc {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  text-align: center;
}
</style>
