<script setup lang="ts">
/**
 * Dashboard 首页 — 环绕双翼镜像对称布局
 * 作者: TanXiang
 */
import { onMounted, onUnmounted, computed, ref } from "vue";
import { useConnectionStore } from "@/stores/connection.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useSettingsStore } from "@/stores/settings.store";
import { useFluidWave } from "@/composables/useFluidWave";
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { info as logInfo, error as logError, warn as logWarn } from "@tauri-apps/plugin-log";
import SpeedChart from "@/components/charts/SpeedChart.vue";

const router = useRouter();
const connectionStore = useConnectionStore();
const proxyStore = useProxyStore();
const settingsStore = useSettingsStore();

// 代理流量接管总开关 (ON / OFF)
const proxyActive = ref(true);
const operating = ref(false);

const {
  smoothDownloadSpeed,
  activeConnectionCount,
  totalDownload,
  totalUpload
} = storeToRefs(connectionStore);

const { rotationDeg } = useFluidWave({ speedBps: smoothDownloadSpeed });

// 能量核在不同代理分流模式下的主题色彩配置表 (配置模式，便于维护与扩展)
const CORE_GLOW_THEMES = {
  global: {
    ring: {
      background: "linear-gradient(135deg, #f093fb 0%, #f5576c 100%)",
      boxShadow: "0 0 24px rgba(245, 87, 108, 0.45), inset 0 0 12px rgba(245, 87, 108, 0.3)",
    },
    text: {
      color: "#f093fb",
      textShadow: "0 0 8px rgba(240, 147, 251, 0.6)",
    },
    breathing: "rgba(240, 147, 251, 0.4)"
  },
  direct: {
    ring: {
      background: "linear-gradient(135deg, #43e97b 0%, #38f9d7 100%)",
      boxShadow: "0 0 24px rgba(67, 233, 123, 0.45), inset 0 0 12px rgba(67, 233, 123, 0.3)",
    },
    text: {
      color: "#43e97b",
      textShadow: "0 0 8px rgba(67, 233, 123, 0.6)",
    },
    breathing: "rgba(67, 233, 123, 0.4)"
  },
  rule: {
    ring: {
      background: "linear-gradient(135deg, #00f2fe 0%, #4facfe 100%)",
      boxShadow: "0 0 24px rgba(0, 242, 254, 0.45), inset 0 0 12px rgba(0, 242, 254, 0.3)",
    },
    text: {
      color: "#00f2fe",
      textShadow: "0 0 8px rgba(0, 242, 254, 0.6)",
    },
    breathing: "rgba(0, 242, 254, 0.4)"
  }
} as const;

// 针对不同的分流规则和运行状态计算不同的光圈和文字阴影样式
const coreGlowStyle = computed(() => {
  if (!proxyActive.value) {
    return {
      ring: {
        background: "var(--energy-idle)",
        boxShadow: "none",
      },
      text: {
        color: "var(--text-secondary)",
        textShadow: "none",
      }
    };
  }
  const mode = proxyStore.proxyMode as keyof typeof CORE_GLOW_THEMES;
  return CORE_GLOW_THEMES[mode] || CORE_GLOW_THEMES.rule;
});

// 计算呼吸灯的光晕阴影颜色，配合 v-bind 实现动态关键帧渲染
const breathingGlowColor = computed(() => {
  if (!proxyActive.value) return "rgba(255, 255, 255, 0.05)";
  const mode = proxyStore.proxyMode as keyof typeof CORE_GLOW_THEMES;
  return CORE_GLOW_THEMES[mode]?.breathing || "rgba(0, 242, 254, 0.4)";
});

// 流量接管双态读写双向绑定
const inboundMode = computed({
  get() {
    return settingsStore.settings.tun_enabled ? "tun" : "system";
  },
  async set(val: "system" | "tun") {
    if (operating.value) return;
    operating.value = true;
    logInfo(`[DashboardView] 用户点击切换网络接管模式为: ${val}`);

    const originalVal = settingsStore.settings.tun_enabled;
    const isTun = val === "tun";
    
    // 乐观更新：立刻在前端呈现开关变动
    settingsStore.settings.tun_enabled = isTun;

    // 开启非阻塞的异步微任务以平滑过渡 UI 选中状态
    setTimeout(async () => {
      try {
        if (!proxyActive.value) {
          logInfo(`[DashboardView] 代理未激活，仅标记设置 tun_enabled = ${isTun}`);
          await invoke("tun_set_enabled", { enabled: isTun });
          return;
        }

        if (val === "system") {
          logInfo("[DashboardView] 切换为普通系统代理模式，正在卸载 TUN...");
          const res: any = await invoke("tun_set_enabled", { enabled: false });
          if (res.success) {
            if (proxyStore.proxyMode === "direct") {
              await proxyStore.changeProxyMode("rule");
            }
          } else {
             settingsStore.settings.tun_enabled = originalVal; // 回滚
          }
        } else if (val === "tun") {
          logInfo("[DashboardView] 切换为 TUN 虚拟网卡模式...");
          const res: any = await invoke("tun_set_enabled", { enabled: true });
          if (!res.success) {
            logWarn(`[DashboardView] 启动 TUN 失败: ${res.error}，提示用户一键提权安装服务...`);
            const confirmInstall = confirm(
              "启用 TUN 虚拟网卡需要管理员权限来安装静默提权组件。\n\n是否允许程序执行一键安装？(此后开启 TUN 将永久免弹窗免重启)"
            );
            if (confirmInstall) {
              logInfo("[DashboardView] 用户同意提权安装服务，开始调用 service_install...");
              const installRes: any = await invoke("service_install");
              if (installRes.success) {
                logInfo("[DashboardView] 服务安装成功，重新尝试启动 TUN...");
                const retryRes: any = await invoke("tun_set_enabled", { enabled: true });
                if (retryRes.success) {
                  logInfo("[DashboardView] 重试启动 TUN 成功");
                  await proxyStore.changeProxyMode("direct");
                  return;
                } else {
                  logError(`[DashboardView] 服务安装后，重试启动 TUN 依然失败: ${retryRes.error}`);
                }
              } else {
                logError(`[DashboardView] 提权服务安装失败: ${installRes.error}`);
              }
            }
            logWarn("[DashboardView] 启动 TUN 失败，静默回滚乐观状态");
            settingsStore.settings.tun_enabled = originalVal; // 回滚
            return;
          }
          logInfo("[DashboardView] 启动 TUN 成功");
          await proxyStore.changeProxyMode("direct");
        }
      } catch (e: any) {
        logError(`[DashboardView] 切换接管模式发生致命错误: ${e.message || e}`);
        settingsStore.settings.tun_enabled = originalVal; // 回滚
      } finally {
        operating.value = false;
      }
    }, 50);
  }
});

// 大圆环点击：一键接管网络 / 完全注销释放 Windows 代理 (网络自救总开关)
async function toggleProxy() {
  if (operating.value) return;
  operating.value = true;

  const nextActive = !proxyActive.value;
  // 乐观更新：立刻在前端呈现开关变动
  proxyActive.value = nextActive;

  setTimeout(async () => {
    try {
      if (nextActive) {
        if (settingsStore.settings.tun_enabled) {
          const res: any = await invoke("tun_set_enabled", { enabled: true });
          if (res.success) {
            settingsStore.settings.tun_enabled = true;
            await proxyStore.changeProxyMode("direct");
          } else {
            const confirmInstall = confirm(
              "启用 TUN 虚拟网卡需要管理员权限来安装静默提权组件。\n\n是否允许程序执行一键安装？(此后开启 TUN 将永久免弹窗免重启)"
            );
            if (confirmInstall) {
              const installRes: any = await invoke("service_install");
              if (installRes.success) {
                const retryRes: any = await invoke("tun_set_enabled", { enabled: true });
                if (retryRes.success) {
                  settingsStore.settings.tun_enabled = true;
                  await proxyStore.changeProxyMode("direct");
                  return;
                }
              }
            }
            proxyActive.value = false;
            settingsStore.settings.tun_enabled = false;
          }
        } else {
          await invoke("tun_set_enabled", { enabled: false });
          settingsStore.settings.tun_enabled = false;
          if (proxyStore.proxyMode === "direct") {
            await proxyStore.changeProxyMode("rule");
          }
        }
      } else {
        await settingsStore.updateSettings({ tun_enabled: false, proxy_mode: "direct" });
        settingsStore.settings.tun_enabled = false;
        proxyStore.$patch({ proxyMode: "direct" });
      }
    } catch (e) {
      console.error("开关代理错误: ", e);
      proxyActive.value = !nextActive; // 回滚乐观更新
    } finally {
      operating.value = false;
    }
  }, 50);
}

function changeMode(mode: "global" | "rule" | "direct") {
  if (operating.value) return;
  operating.value = true;
  setTimeout(async () => {
    try {
      await proxyStore.changeProxyMode(mode);
    } finally {
      operating.value = false;
    }
  }, 50);
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

let statusTimer: ReturnType<typeof setInterval> | null = null;

async function checkRunningStatus() {
  if (operating.value) return; // 在变更过程中避免覆盖乐观状态
  try {
    const runningRes: any = await invoke("core_query_running");
    if (runningRes.success) {
      proxyActive.value = runningRes.data;
    }
  } catch (e: any) {
    console.error("轮询内核状态异常:", e);
  }
}

onMounted(async () => {
  await settingsStore.fetchSettings();
  proxyStore.fetchGroups();
  
  try {
    await checkRunningStatus();
    logInfo(`[DashboardView] 初始化核心运行状态: ${proxyActive.value ? '运行中' : '未运行'}`);
  } catch (e: any) {
    proxyActive.value = false;
    logError(`[DashboardView] 获取核心运行状态异常: ${e.message || e}`);
  }

  // 启动定时轮询，每 3 秒同步一次状态
  statusTimer = setInterval(checkRunningStatus, 3000);
});

onUnmounted(() => {
  if (statusTimer) {
    clearInterval(statusTimer);
  }
});
</script>

<template>
  <div class="dashboard-layout">
    <!-- 上部：“两翼对称”三栏全息悬浮大格局 -->
    <div class="top-panel-row" :class="{ 'idle-layout': !proxyActive }">
      <!-- 左翼：流量接管与控制 (至简双胶囊) -->
      <div v-if="proxyActive" class="control-wing">
        <!-- 胶囊 1: 流量接管双态切换 (System/TUN) -->
        <div class="stat-pill">
          <span class="pill-label">流量接管</span>
          <div class="mode-selector inbound-selector">
            <button
              v-for="mode in ['system', 'tun']"
              :key="mode"
              class="mode-btn"
              :class="{ active: inboundMode === mode }"
              :disabled="operating"
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
              :disabled="operating"
              @click="changeMode(mode as any)"
            >
              {{ mode === 'global' ? '全局' : mode === 'rule' ? '规则' : '直连' }}
            </button>
          </div>
        </div>
      </div>

      <!-- 中央：旋转能量核 (视觉绝对重心) -->
      <div class="energy-wing" :class="{ 'full-center': !proxyActive }">
        <div
          class="energy-core"
          :class="{ connected: proxyActive }"
          @click="toggleProxy"
          title="点击开启或释放系统流量接管"
        >
          <div
            class="energy-ring"
            :style="[
              { transform: proxyActive ? 'rotate(' + rotationDeg + 'deg)' : 'none' },
              coreGlowStyle.ring
            ]"
          >
            <div class="energy-inner">
              <span v-if="proxyActive" class="energy-status-text" :style="coreGlowStyle.text">CONNECTED</span>
              <span v-else class="energy-status-text idle">TAP TO CONNECT</span>
            </div>
          </div>
        </div>

        <!-- 关闭代理时的极简科技神盾详情 -->
        <div v-if="!proxyActive" class="shield-tips-container">
          <div class="shield-icon">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="shield-svg">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
            </svg>
          </div>
          <div class="shield-title">Auroweave 安全防线已静默</div>
          <div class="shield-desc">
            Windows 系统代理与 TUN 虚拟网卡已安全释放。点击中心核一键激活网络全息守护。
          </div>
        </div>
      </div>

      <!-- 右翼：流量统计与数据 (垂直对齐镜像) -->
      <div v-if="proxyActive" class="stats-wing">
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
  transition: background 0.6s ease-in-out, box-shadow 0.6s ease-in-out, transform var(--duration-normal) var(--ease-out);
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
  transition: color 0.6s ease-in-out, text-shadow 0.6s ease-in-out;
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
    filter: drop-shadow(0 0 8px v-bind(breathingGlowColor));
  }
  50% {
    transform: scale(1.025); /* 极细微优雅的形体收缩 */
    filter: drop-shadow(0 0 20px v-bind(breathingGlowColor)); /* 吞吐光晕阴影 */
  }
  100% {
    transform: scale(1);
    filter: drop-shadow(0 0 8px v-bind(breathingGlowColor));
  }
}

/* ---- 关闭代理时（Idle 状态）布局微调与科技防线包装 ---- */
.top-panel-row.idle-layout {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  flex: 1;
  height: 100%;
  padding-top: 40px; /* 精调整体向下居中的舒适上边距 */
}

.energy-wing.full-center {
  flex-direction: column;
  height: auto;
  gap: 36px;
  justify-content: center;
  align-items: center;
}

.shield-tips-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  max-width: 420px;
  opacity: 0;
  transform: translateY(12px);
  animation: fadeInShield 0.8s forwards var(--ease-out);
}

.shield-icon {
  width: 44px;
  height: 44px;
  color: var(--text-tertiary);
  margin-bottom: 12px;
  display: flex;
  justify-content: center;
  align-items: center;
  background: var(--layer-1);
  border: 1px solid var(--border-normal);
  border-radius: 50%;
  box-shadow: var(--shadow-sm);
  transition: all 0.5s ease-in-out;
}

.shield-svg {
  width: 20px;
  height: 20px;
}

/* 兄弟选择器联动：当大圆环被悬浮时，下方的盾牌组件亮起呼吸青光！ */
.energy-core:hover + .shield-tips-container .shield-icon {
  color: var(--accent-cyan);
  border-color: var(--accent-cyan-glow);
  box-shadow: 0 0 16px var(--accent-cyan-glow);
}

.shield-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
  margin-bottom: 8px;
  letter-spacing: 1px;
}

.shield-desc {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  line-height: 1.6;
}

@keyframes fadeInShield {
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>
