<script setup lang="ts">
/**
 * Dashboard 首页 — 主入口
 * 作者: TanXiang
 *
 * 职责：布局拼装、状态绑定、Hook 协调
 */
import { storeToRefs } from "pinia";
import { computed, onActivated, watch } from "vue";
import { useRouter } from "vue-router";
import { useConnectionStore } from "@/stores/connection.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { useSettingsStore } from "@/stores/settings.store";
import { useSpeedtestStore } from "@/stores/speedtest.store";
import { useFluidWave } from "@/composables/useFluidWave";

import EnergyCore from "./components/EnergyCore.vue";
import ControlWing from "./components/ControlWing.vue";
import StatsWing from "./components/StatsWing.vue";
import SpeedChart from "@/components/charts/SpeedChart.vue";

import { useProxyToggle } from "./hooks/useProxyToggle";
import { useInboundMode } from "./hooks/useInboundMode";
import { useCoreStatus } from "./hooks/useCoreStatus";
import { useEgressInfo } from "./hooks/useEgressInfo";

const router = useRouter();
const connectionStore = useConnectionStore();
const proxyStore = useProxyStore();
const subStore = useSubscriptionStore();
const speedtestStore = useSpeedtestStore();
const settingsStore = useSettingsStore();

const { smoothDownloadSpeed, activeConnectionCount, totalDownload, totalUpload } =
  storeToRefs(connectionStore);

// === Hook 初始化 ===

const { proxyActive, coreStarting, operating, recentToggleUntil, toggleProxy, changeMode } = useProxyToggle();

const { inboundMode } = useInboundMode({ proxyActive, operating });

useCoreStatus({ proxyActive, coreStarting, operating, recentToggleUntil });

// === 首页出口与节点信息 ===

// 当前工作节点（含负载均衡/自动组递归解析）
const { workingNodeName } = storeToRefs(proxyStore);

// 当前节点延迟：测速 store 的 latencyMap（用户点过测延迟/批量测延迟后缓存）
const currentNodeLatency = computed(
  () => speedtestStore.latencyMap[workingNodeName.value] ?? undefined
);

// 出口 IP + 归属地 + 国旗（激活时探测一次，节点切换防抖重探）
const { egress, egressLoading, refresh: refreshEgress } = useEgressInfo(workingNodeName);

// 首页胶囊显隐开关（设置-首页显示；字段缺失视为开启，兼容旧 settings.json）
const dash = storeToRefs(settingsStore).settings;
const showConnections = computed(() => dash.value.dashboard_show_connections !== false);
const showCurrentNode = computed(() => dash.value.dashboard_show_current_node !== false);
const showEgressIp = computed(() => dash.value.dashboard_show_egress_ip !== false);
const showTotalTraffic = computed(() => dash.value.dashboard_show_total_traffic !== false);

// === 流体波浪旋转角度 ===

const { rotationDeg } = useFluidWave({ speedBps: smoothDownloadSpeed });

// === 导航 ===

function navigate(path: string) {
  router.push(path);
}

// KeepAlive 激活时拉取订阅列表，供 IdleTipsPanel 判断是否显示订阅入口按钮
// （避免 setup 顶层执行导致缓存后不再刷新）
// 出口探测同样在激活时触发（代理开着才有意义；出口胶囊被关闭时跳过）
onActivated(() => {
  subStore.fetchAll();
  if (proxyActive.value && showEgressIp.value) refreshEgress();
});

// 代理从关到开时补一次探测（首次进入时 proxyActive 可能尚未就绪）
watch(proxyActive, (active) => {
  if (active && showEgressIp.value) refreshEgress();
});
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
        :core-starting="coreStarting"
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
        :current-node="workingNodeName === '直连' ? undefined : workingNodeName"
        :current-node-latency="currentNodeLatency"
        :egress-info="egress"
        :egress-loading="egressLoading"
        :show-connections="showConnections"
        :show-current-node="showCurrentNode"
        :show-egress-ip="showEgressIp"
        :show-total-traffic="showTotalTraffic"
        @navigate="navigate"
        @refresh-egress="refreshEgress"
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
