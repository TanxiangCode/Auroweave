<script setup lang="ts">
/**
 * 全局控制胶囊 (Control Capsule)
 * 作者: TanXiang
 *
 * 视觉设计：
 * - 紧凑超精致毛玻璃浮层
 * - 遵守防误触设计：设置与关闭按钮物理隔离 > 60px
 * - macOS 环境下隐藏右侧 ─/⬜/<BaseIcon name="X" :size="12" /> 按钮 (由 macOS 原生红绿灯接管)
 */
import { ref, onMounted } from "vue";
import { useRoute, useRouter } from "vue-router";
import BaseIcon from "@/components/common/BaseIcon.vue";


import {
  SETTINGS_CLOSE_MIN_GAP_PX,
  WINDOW_DEFAULT_WIDTH,
  WINDOW_DEFAULT_HEIGHT,
} from "@/constants";

const router = useRouter();
const route = useRoute();

const isMaximized = ref(false);
const isMac = ref(false);

onMounted(async () => {
  isMac.value = navigator.userAgent.toLowerCase().includes("mac");
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    isMaximized.value = await appWindow.isMaximized();

    appWindow.onResized(async () => {
      isMaximized.value = await appWindow.isMaximized();
    });
  } catch (e) {
    console.log("Window control capsule: Tauri API not available, running in browser preview mode.", e);
  }
});

async function handleResetSize() {
  try {
    const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    await appWindow.setSize(new LogicalSize(WINDOW_DEFAULT_WIDTH, WINDOW_DEFAULT_HEIGHT));
    await appWindow.center();
  } catch (e) {
    console.log(`Reset size to: ${WINDOW_DEFAULT_WIDTH}x${WINDOW_DEFAULT_HEIGHT}`, e);
  }
}

async function handleMinimize() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().minimize();
  } catch (e) {
    console.log("Minimize window (mock)", e);
  }
}

async function handleToggleMaximize() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    await appWindow.toggleMaximize();
    isMaximized.value = await appWindow.isMaximized();
  } catch (e) {
    isMaximized.value = !isMaximized.value;
    console.log("Toggle maximize window (mock)", e);
  }
}

async function handleClose() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  } catch (e) {
    console.log("Close window (mock)", e);
  }
}

function handleSettingsClick() {
  if (route.path === "/settings") {
    router.back();
  } else {
    router.push("/settings");
  }
}
</script>

<template>
  <div
    class="control-capsule glass-effect"
    title="双击控制胶囊：重置窗口为 960x640 黄金尺寸并居中"
    @dblclick="handleResetSize"
  >
    <!-- 核心导航按钮群 -->
    <div class="nav-buttons">
      <!-- 首页 -->
      <button
        class="capsule-btn nav-btn"
        :class="{ active: route.path === '/' || route.path === '/dashboard' }"
        title="首页"
        @click="router.push('/')"
      >
        <BaseIcon name="Compass" :size="13" />
      </button>
      
      <!-- 代理大厅 -->
      <button
        class="capsule-btn nav-btn"
        :class="{ active: route.path === '/proxies' }"
        title="代理大厅"
        @click="router.push('/proxies')"
      >
        <BaseIcon name="Radio" :size="13" />
      </button>
      
      <!-- 订阅中心 (一级核心模块) -->
      <button
        class="capsule-btn nav-btn"
        :class="{ active: route.path === '/subscriptions' }"
        title="订阅中心"
        @click="router.push('/subscriptions')"
      >
        <BaseIcon name="Rss" :size="13" />
      </button>

      <!-- 分流规则 -->
      <button
        class="capsule-btn nav-btn"
        :class="{ active: route.path === '/routing' }"
        title="分流规则"
        @click="router.push('/routing')"
      >
        <BaseIcon name="GitFork" :size="13" />
      </button>
      
      <!-- 安全审计 -->
      <button
        class="capsule-btn nav-btn"
        :class="{ active: route.path === '/audit' }"
        title="安全审计"
        @click="router.push('/audit')"
      >
        <BaseIcon name="ShieldCheck" :size="13" />
      </button>
    </div>

    <!-- 设置按钮 (在设置页内高亮) -->
    <button
      class="capsule-btn settings-btn"
      :class="{ active: route.path === '/settings' }"
      title="偏好设置"
      @click="handleSettingsClick"
    >
      <BaseIcon name="Settings" :size="13" />
    </button>


    <!-- 非 macOS 平台下的窗口控制按钮群 (设置与关闭分割间距遵循 SETTINGS_CLOSE_MIN_GAP_PX 防误触) -->
    <template v-if="!isMac">
      <div
        class="capsule-divider"
        :style="{ margin: `0 ${Math.max(6, SETTINGS_CLOSE_MIN_GAP_PX / 10)}px` }"
      />

      <div class="window-controls">
        <button class="capsule-btn minimize-btn" title="最小化" @click="handleMinimize">
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="5" y1="12" x2="19" y2="12"></line>
          </svg>
        </button>

        <button
          class="capsule-btn maximize-btn"
          :title="isMaximized ? '还原' : '最大化'"
          @click="handleToggleMaximize"
        >
          <svg v-if="!isMaximized" xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
          </svg>
          <svg v-else xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="9" width="12" height="12" rx="2"></rect>
            <path d="M9 9V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-4"></path>
          </svg>
        </button>

        <button class="capsule-btn close-btn" title="关闭" @click="handleClose">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.control-capsule {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 3px 6px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  box-shadow: var(--shadow-sm);
  transition: all var(--duration-normal) var(--ease-out);
  /* 无边框模式下：明确声明整个胶囊及其子元素不参与拖拽 */
  -webkit-app-region: no-drag;
  app-region: no-drag;
}

.control-capsule:hover {
  border-color: var(--border-accent);
}

.nav-buttons {
  display: flex;
  align-items: center;
  gap: 3px;
}

.capsule-divider {
  width: 1px;
  height: 12px;
  background: var(--surface-raised);
  margin: 0 4px;
}

.window-controls {
  display: flex;
  align-items: center;
  gap: 2px;
}

.capsule-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
  /* 强制覆盖：确保每个按钮都不被拖拽区域拦截 */
  -webkit-app-region: no-drag;
  app-region: no-drag;
}

.capsule-btn:hover {
  background: var(--surface-hover);
  color: var(--text-primary);
  transform: scale(1.05);
}

.capsule-btn.active {
  background: var(--accent-blue-glow);
  color: var(--accent-blue);
}

.close-btn:hover {
  background: var(--accent-red);
  color: var(--text-on-accent);
}
</style>
