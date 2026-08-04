<script setup lang="ts">
/**
 * Dashboard 首页 — 主入口
 * 作者: TanXiang
 *
 * 职责：布局拼装、状态绑定、Hook 协调
 */
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";
import { useConnectionStore } from "@/stores/connection.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useFluidWave } from "@/composables/useFluidWave";

import EnergyCore from "./components/EnergyCore.vue";
import ControlWing from "./components/ControlWing.vue";
import StatsWing from "./components/StatsWing.vue";
import SpeedChart from "@/components/charts/SpeedChart.vue";

import { useProxyToggle } from "./hooks/useProxyToggle";
import { useInboundMode } from "./hooks/useInboundMode";
import { useCoreStatus } from "./hooks/useCoreStatus";

const router = useRouter();
const connectionStore = useConnectionStore();
const proxyStore = useProxyStore();

const { smoothDownloadSpeed, activeConnectionCount, totalDownload, totalUpload } =
  storeToRefs(connectionStore);

// === Hook 初始化 ===

const { proxyActive, operating, toggleProxy, changeMode } = useProxyToggle();

const { inboundMode } = useInboundMode({ proxyActive, operating });

useCoreStatus({ proxyActive, operating });

// === 流体波浪旋转角度 ===

const { rotationDeg } = useFluidWave({ speedBps: smoothDownloadSpeed });

// === 导航 ===

function navigate(path: string) {
  router.push(path);
}
</script>

<template>
  <div class="dashboard-layout">
    <!-- 上部：两翼对称三栏 -->
    <div class="top-panel-row" :class="{ 'idle-layout': !proxyActive }">
      <!-- 左翼：控制 -->
      <ControlWing
        v-if="proxyActive"
        :inbound-mode="inboundMode"
        :proxy-mode="proxyStore.proxyMode"
        :operating="operating"
        @update:inbound-mode="inboundMode = $event as any"
        @change-mode="changeMode"
      />

      <!-- 中央：能量核 -->
      <EnergyCore
        :proxy-active="proxyActive"
        :proxy-mode="proxyStore.proxyMode"
        :rotation-deg="rotationDeg"
        @toggle="toggleProxy"
      />

      <!-- 右翼：统计 -->
      <StatsWing
        v-if="proxyActive"
        :active-connection-count="activeConnectionCount"
        :total-download="totalDownload"
        :total-upload="totalUpload"
        @navigate="navigate"
      />
    </div>

    <!-- 下部：实时折线图 -->
    <div v-if="proxyActive" class="bottom-panel-row">
      <SpeedChart />
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
  overflow: hidden;
}

.top-panel-row {
  display: grid;
  grid-template-columns: 320px 1fr 280px;
  gap: 24px;
  align-items: center;
  width: 100%;
  flex: 1;
}

/* 关闭代理时的布局 */
.top-panel-row.idle-layout {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  flex: 1;
  height: 100%;
  padding-top: 40px;
}

.bottom-panel-row {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
  max-height: 200px;
  margin-top: auto;
}

.bottom-panel-row :deep(.speed-chart-card) {
  border-radius: 24px !important;
  border-color: var(--border-normal);
  height: 100%;
  box-shadow: var(--shadow-sm);
}
</style>
