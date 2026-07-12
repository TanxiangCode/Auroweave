<script setup lang="ts">
/**
 * Dashboard 首页 — 环绕双翼镜像对称布局
 * 作者: TanXiang
 */
import { onMounted, computed } from "vue";
import { useConnectionStore } from "@/stores/connection.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useSettingsStore } from "@/stores/settings.store";
import { useFluidWave } from "@/composables/useFluidWave";
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";
import SpeedChart from "@/components/charts/SpeedChart.vue";
import SvgIcon from "@/components/common/SvgIcon.vue";

const router = useRouter();
const connectionStore = useConnectionStore();
const proxyStore = useProxyStore();
const settingsStore = useSettingsStore();

const {
  rawDownloadSpeed,
  rawUploadSpeed,
  smoothDownloadSpeed,
  activeConnectionCount,
  totalDownload,
  totalUpload
} = storeToRefs(connectionStore);

const { rotationDeg } = useFluidWave({ speedBps: smoothDownloadSpeed });

// 流量接管三态读写双向绑定
const inboundMode = computed({
  get() {
    const tun = settingsStore.settings.tun_enabled;
    const isDirect = proxyStore.proxyMode === "direct";
    if (tun && isDirect) return "tun";
    if (tun && !isDirect) return "mixed";
    return "system";
  },
  async set(val: "system" | "tun" | "mixed") {
    if (val === "system") {
      await settingsStore.updateSettings({ tun_enabled: false });
      if (proxyStore.proxyMode === "direct") {
        await proxyStore.changeProxyMode("rule");
      }
    } else if (val === "tun") {
      await settingsStore.updateSettings({ tun_enabled: true });
      await proxyStore.changeProxyMode("direct");
    } else if (val === "mixed") {
      await settingsStore.updateSettings({ tun_enabled: true });
      if (proxyStore.proxyMode === "direct") {
        await proxyStore.changeProxyMode("rule");
      }
    }
  }
});

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

onMounted(async () => {
  await settingsStore.fetchSettings();
  proxyStore.fetchGroups();
});
</script>

<template>
  <div class="dashboard-layout">
    <!-- 上部：“两翼对称”三栏全息悬浮大格局 -->
    <div class="top-panel-row">
      <!-- 左翼：流量接管与控制 -->
      <div class="control-wing">
        <!-- 胶囊 1: 流量接管三态切换 (System/TUN/Mixed) -->
        <div class="stat-pill">
          <span class="pill-label">流量接管</span>
          <div class="mode-selector inbound-selector">
            <button
              v-for="mode in ['system', 'tun', 'mixed']"
              :key="mode"
              class="mode-btn"
              :class="{ active: inboundMode === mode }"
              @click="inboundMode = mode as any"
            >
              {{ mode === 'system' ? '系统' : mode === 'tun' ? 'TUN' : '混合' }}
            </button>
          </div>
        </div>

        <!-- 胶囊 2: 分流模式三态切换 (Global/Rule/Direct) -->
        <div class="stat-pill">
          <span class="pill-label">分流规则</span>
          <div class="mode-selector rule-selector">
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

        <!-- 胶囊 3: 核心引擎状态与状态同步 -->
        <div class="stat-pill clickable-pill" @click="proxyStore.fetchGroups" title="点击手动同步刷新引擎数据">
          <span class="pill-label">核心引擎</span>
          <div class="engine-status-row">
            <span class="engine-active-tag">
              <span class="status-dot-indicator active"></span>
              ACTIVE
            </span>
            <SvgIcon name="refresh" :size="12" class="refresh-icon-spin" />
          </div>
        </div>
      </div>

      <!-- 中央：旋转能量核 (视觉绝对重心) -->
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

      <!-- 右翼：流量统计与历史 (垂直层叠镜像) -->
      <div class="stats-wing">
        <!-- 胶囊 1: 活动连接 -> 点击跳转安全审计 -->
        <div
          class="stat-pill clickable-pill"
          @click="router.push('/audit')"
          title="点击查看实时连接审计"
        >
          <span class="pill-label">活动连接</span>
          <span class="pill-val cyan-glow">{{ activeConnectionCount }} 条</span>
        </div>

        <!-- 胶囊 2: 累计下载 -> 点击跳转流量统计 -->
        <div
          class="stat-pill clickable-pill"
          @click="router.push('/stats')"
          title="点击查看详细流量统计"
        >
          <span class="pill-label">累计下载</span>
          <span class="pill-val">{{ formatBytes(totalDownload) }}</span>
        </div>

        <!-- 胶囊 3: 累计上传 -> 点击跳转流量统计 -->
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

    <!-- 下部：实时折线图托底座 -->
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

/* ---- 上部：“两翼对称”三栏全息悬浮大格局 ---- */
.top-panel-row {
  display: grid;
  grid-template-columns: 320px 1fr 260px;
  gap: 24px;
  align-items: center;
  width: 100%;
  height: 260px; /* 锁死上部高度，保证对称呼吸感 */
}

/* 移除 control-wing 和 stats-wing 的 glass-effect 大包装背景，改为完全高透悬浮 */
.control-wing, .stats-wing {
  display: flex;
  flex-direction: column;
  gap: 16px;
  justify-content: center;
  height: 100%;
}

/* ---- 化方为圆：高透胶囊药丸 (Sleek Stadium Pill) ---- */
.stat-pill {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full); /* 彻底的胶囊圆形跑道角，告别方圆违和 */
  height: 48px; /* 锁定高度，保证两侧垂直对齐极其工整 */
  transition: all var(--duration-fast) var(--ease-out);
}

.stat-pill.clickable-pill {
  cursor: pointer;
}

/* 向内磁吸式 hover 位移动效 (左翼向右，右翼向左) */
.control-wing .stat-pill.clickable-pill:hover {
  background: var(--layer-2);
  border-color: var(--accent-blue);
  box-shadow: 0 0 12px var(--accent-blue-glow);
  transform: translateX(4px); /* 向右朝能量核方向微微聚拢 */
}

.stats-wing .stat-pill.clickable-pill:hover {
  background: var(--layer-2);
  border-color: var(--accent-blue);
  box-shadow: 0 0 12px var(--accent-blue-glow);
  transform: translateX(-4px); /* 向左朝能量核方向微微聚拢 */
}

.pill-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--weight-bold);
  letter-spacing: 0.5px;
}

.pill-val {
  font-size: var(--text-xs);
  color: var(--text-primary);
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
}

.pill-val.cyan-glow {
  color: var(--accent-cyan);
  text-shadow: 0 0 6px var(--accent-cyan-glow);
}

/* ---- 模式选择器 (Mode Selector) 全圆角大统一 ---- */
.mode-selector {
  display: flex;
  background: var(--layer-2);
  padding: 2px;
  border-radius: var(--radius-full); /* 完全圆润 */
  gap: 2px;
}

/* 接管模式包含 3 项，设定总宽 */
.inbound-selector {
  width: 160px;
}

/* 分流模式包含 3 项，设定总宽 */
.rule-selector {
  width: 140px;
}

.mode-btn {
  flex: 1;
  padding: 4px 0;
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 10px;
  font-weight: var(--weight-semibold);
  border-radius: var(--radius-full);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.mode-btn:hover {
  color: var(--text-primary);
}

.mode-btn.active {
  background: var(--layer-1);
  color: var(--accent-cyan);
  font-weight: var(--weight-bold);
  box-shadow: 0 0 8px var(--accent-cyan-glow);
}

/* 规则选择按钮激活态用蓝光呼应 */
.rule-selector .mode-btn.active {
  color: var(--accent-blue);
  box-shadow: 0 0 8px var(--accent-blue-glow);
}

/* ---- 核心引擎状态与刷新按钮 ---- */
.engine-status-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.engine-active-tag {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 10px;
  font-weight: var(--weight-bold);
  color: var(--accent-green);
  background: var(--accent-green-glow);
  padding: 2px 8px;
  border-radius: var(--radius-full);
}

.status-dot-indicator {
  display: inline-block;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--text-tertiary);
}

.status-dot-indicator.active {
  background: var(--accent-green);
  box-shadow: 0 0 6px var(--accent-green);
}

.refresh-icon-spin {
  color: var(--text-tertiary);
  transition: transform 0.4s var(--ease-out);
}

.stat-pill:hover .refresh-icon-spin {
  color: var(--accent-green);
  transform: rotate(180deg); /* 鼠标移入自动旋转，灵动非凡 */
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

/* ---- 下部：折线图底座 ---- */
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

/* 磨砂玻璃底座 — 提升圆角至 24px，化棱角为圆润 */
.glass-effect {
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: 24px; /* 升级为 24px，视觉圆润过渡 */
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
