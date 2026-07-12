<script setup lang="ts">
/**
 * Dashboard 首页 — 环绕双翼镜像对称布局
 * 作者: TanXiang
 */
import { onMounted, computed, ref } from "vue";
import { useConnectionStore } from "@/stores/connection.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useSettingsStore } from "@/stores/settings.store";
import { useFluidWave } from "@/composables/useFluidWave";
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import SpeedChart from "@/components/charts/SpeedChart.vue";

const router = useRouter();
const connectionStore = useConnectionStore();
const proxyStore = useProxyStore();
const settingsStore = useSettingsStore();

// 代理流量接管总开关 (ON / OFF)
const proxyActive = ref(true);

const {
  smoothDownloadSpeed,
  activeConnectionCount,
  totalDownload,
  totalUpload
} = storeToRefs(connectionStore);

const { rotationDeg } = useFluidWave({ speedBps: smoothDownloadSpeed });

// 流量接管双态读写双向绑定
const inboundMode = computed({
  get() {
    return settingsStore.settings.tun_enabled ? "tun" : "system";
  },
  async set(val: "system" | "tun") {
    // 若总开关未开启，仅静默记录设置，不进行物理系统代理变动
    if (!proxyActive.value) {
      await settingsStore.updateSettings({ tun_enabled: val === "tun" });
      return;
    }

    if (val === "system") {
      await settingsStore.updateSettings({ tun_enabled: false });
      // 开启 Windows IE 系统代理
      await invoke("sysproxy_set", { enabled: true, port: settingsStore.settings.mixed_port });
      if (proxyStore.proxyMode === "direct") {
        await proxyStore.changeProxyMode("rule");
      }
    } else if (val === "tun") {
      // 开启 TUN 虚拟网卡接管
      const res = await settingsStore.updateSettings({ tun_enabled: true });
      if (!res.success) {
        // UAC 提权重启防御自动申请！
        const confirmRestart = confirm(
          "启用 TUN 虚拟网卡需要管理员/UAC 权限。\n\n是否允许程序自动以管理员身份提权重启？"
        );
        if (confirmRestart) {
          await invoke("app_restart_as_admin");
        }
        
        // 若取消或重启中，安全自动回滚为系统代理接管，防范闪退和断网
        await settingsStore.updateSettings({ tun_enabled: false });
        await invoke("sysproxy_set", { enabled: true, port: settingsStore.settings.mixed_port });
        if (proxyStore.proxyMode === "direct") {
          await proxyStore.changeProxyMode("rule");
        }
        return;
      }
      // 成功拉起 TUN，注销 Windows 系统代理以求纯净
      await invoke("sysproxy_set", { enabled: false, port: settingsStore.settings.mixed_port });
      await proxyStore.changeProxyMode("direct");
    }
  }
});

// 大圆环点击：一键接管网络 / 完全注销释放 Windows 代理 (网络自救总开关)
async function toggleProxy() {
  proxyActive.value = !proxyActive.value;
  
  if (proxyActive.value) {
    // 一键开机，恢复流量接管
    if (settingsStore.settings.tun_enabled) {
      // 激活 TUN，并进行 UAC 权限保护
      const res = await settingsStore.updateSettings({ tun_enabled: true });
      if (res.success) {
        await invoke("sysproxy_set", { enabled: false, port: settingsStore.settings.mixed_port });
        await proxyStore.changeProxyMode("direct");
      } else {
        const confirmRestart = confirm(
          "启用 TUN 虚拟网卡需要管理员/UAC 权限。\n\n是否允许程序自动以管理员身份提权重启？"
        );
        if (confirmRestart) {
          await invoke("app_restart_as_admin");
        }
        proxyActive.value = false;
        await settingsStore.updateSettings({ tun_enabled: false });
        await invoke("sysproxy_set", { enabled: false, port: 0 });
      }
    } else {
      // 激活 Windows 系统代理
      await settingsStore.updateSettings({ tun_enabled: false });
      await invoke("sysproxy_set", { enabled: true, port: settingsStore.settings.mixed_port });
      if (proxyStore.proxyMode === "direct") {
        await proxyStore.changeProxyMode("rule");
      }
    }
  } else {
    // 一键关机，完全释放接管 (注销 Windows IE 代理并关闭 TUN)
    await settingsStore.updateSettings({ tun_enabled: false });
    await invoke("sysproxy_set", { enabled: false, port: 0 });
    // 本地状态更新，不再发送网络请求去调 Clash API (避开因为内核 stop 期间访问 API 报 Toast 切换失败错误)
    proxyStore.$patch({ proxyMode: "direct" });
  }
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
  // 根据 settings 本地配置初始化大开关状态 (在 direct 且 tun 关的状况下为未接管)
  const isDirect = proxyStore.proxyMode === "direct";
  const isTun = settingsStore.settings.tun_enabled;
  if (isDirect && !isTun) {
    proxyActive.value = false;
  } else {
    proxyActive.value = true;
  }
});
</script>

<template>
  <div class="dashboard-layout">
    <!-- 上部：“两翼对称”三栏全息悬浮大格局 -->
    <div class="top-panel-row">
      <!-- 左翼：流量接管与控制 (至简双胶囊) -->
      <div class="control-wing">
        <!-- 胶囊 1: 流量接管双态切换 (System/TUN) -->
        <div class="stat-pill">
          <span class="pill-label">流量接管</span>
          <div class="mode-selector inbound-selector">
            <button
              v-for="mode in ['system', 'tun']"
              :key="mode"
              class="mode-btn"
              :class="{ active: inboundMode === mode }"
              @click="inboundMode = mode as any"
            >
              {{ mode === 'system' ? '系统代理' : 'TUN 网卡' }}
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
      </div>

      <!-- 中央：旋转能量核 (视觉绝对重心) -->
      <div class="energy-wing">
        <div
          class="energy-core"
          :class="{ connected: proxyActive }"
          @click="toggleProxy"
          title="点击开启或释放系统流量接管"
        >
          <div
            class="energy-ring"
            :style="{ transform: proxyActive ? 'rotate(' + rotationDeg + 'deg)' : 'none' }"
          >
            <div class="energy-inner">
              <span v-if="proxyActive" class="energy-status-text">CONNECTED</span>
              <span v-else class="energy-status-text idle">TAP TO CONNECT</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 右翼：流量统计与数据 (垂直对齐镜像) -->
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

        <!-- 胶囊 2: 累计流量 -> 下载/上传合二为一，点击跳转流量统计 -->
        <div
          class="stat-pill clickable-pill"
          @click="router.push('/stats')"
          title="点击查看详细流量统计"
        >
          <span class="pill-label">累计流量</span>
          <span class="pill-val data-combined-val">
            <span class="down-flow">↓ {{ formatBytes(totalDownload) }}</span>
            <span class="divider">|</span>
            <span class="up-flow">↑ {{ formatBytes(totalUpload) }}</span>
          </span>
        </div>
      </div>
    </div>

    <!-- 下部：实时折线图托底座 (消除重复 Header 标题) -->
    <div class="bottom-panel-row">
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
  overflow: hidden; /* 保证在 1020x680 分辨率下完美贴合且不出现滚动条 */
}

/* ---- 上部：“两翼对称”三栏全息悬浮大格局 ---- */
.top-panel-row {
  display: grid;
  grid-template-columns: 320px 1fr 280px; /* 稍微调宽右翼容纳合显流量 */
  gap: 24px;
  align-items: center;
  width: 100%;
  height: 260px; /* 锁死上部高度，保证对称呼吸感 */
}

/* 移除 control-wing 和 stats-wing 的包装背景，完全高透悬浮并底部对齐 */
.control-wing, .stats-wing {
  display: flex;
  flex-direction: column;
  gap: 24px; /* 调宽至 24px，使 2 个胶囊在垂直对齐下高贵舒展 */
  justify-content: flex-end; /* 左右内容底对齐 */
  height: 100%;
  padding-bottom: 10px; /* 精调底部内边距，使胶囊物理底部与大圆环下边缘完全齐平 */
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

/* ---- 累计流量合二为一渲染 ---- */
.data-combined-val {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
}

.down-flow {
  color: var(--accent-cyan);
  text-shadow: 0 0 4px var(--accent-cyan-glow);
}

.up-flow {
  color: var(--accent-purple, #b388ff);
  text-shadow: 0 0 4px rgba(179, 136, 255, 0.25);
}

.divider {
  color: var(--border-strong);
  font-weight: var(--weight-light);
  opacity: 0.5;
}

/* ---- 模式选择器 (Mode Selector) 全圆角大统一 ---- */
.mode-selector {
  display: flex;
  background: var(--layer-2);
  padding: 2px;
  border-radius: var(--radius-full); /* 完全圆润 */
  gap: 2px;
}

/* 接管模式包含 2 项，设定总宽 */
.inbound-selector {
  width: 150px;
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

/* 呼吸发光微缩放动效 (Breathing Scale & Glow) */
.energy-core.connected {
  animation: breathing-core 3s infinite ease-in-out;
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

/* ---- 下部：折线图底座 ---- */
.bottom-panel-row {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0; /* 允许折线图内部自适应伸缩而不溢出 */
}

/* 使用 Vue :deep 穿透修改折线图自带卡片的样式，强制 24px 大圆角与高度撑满 */
.bottom-panel-row :deep(.speed-chart-card) {
  border-radius: 24px !important;
  border-color: var(--border-normal);
  height: 100%;
  box-shadow: var(--shadow-sm);
}

@keyframes breathing-core {
  0% {
    transform: scale(1);
    filter: drop-shadow(0 0 8px rgba(0, 242, 254, 0.25));
  }
  50% {
    transform: scale(1.025); /* 极细微优雅的形体收缩 */
    filter: drop-shadow(0 0 20px rgba(0, 242, 254, 0.55)); /* 吞吐光晕阴影 */
  }
  100% {
    transform: scale(1);
    filter: drop-shadow(0 0 8px rgba(0, 242, 254, 0.25));
  }
}
</style>
