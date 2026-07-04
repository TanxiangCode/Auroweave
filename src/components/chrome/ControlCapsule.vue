<script setup lang="ts">
/**
 * 全局控制与导航胶囊 (Control Capsule)
 * 作者: TanXiang
 *
 * 职责：
 * - 顶层页面导航：主页 🏠、节点 🚀、分流 🛠️、审计 🔍、测速 📊、设置 ⚙️
 * - 窗口控制：最小化 ─、最大化/还原 ⬜、关闭 ✕
 * - 遵守防误触设计与平滑过渡
 */
import { ref, onMounted, computed } from "vue";
import { useRouter, useRoute } from "vue-router";

const router = useRouter();
const route = useRoute();

const isMaximized = ref(false);
const isMac = ref(false);

const isHomePage = computed(() => route.path === "/" || route.path === "/dashboard");

onMounted(async () => {
  isMac.value = navigator.userAgent.toLowerCase().includes("mac");
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    isMaximized.value = await appWindow.isMaximized();

    appWindow.onResized(async () => {
      isMaximized.value = await appWindow.isMaximized();
    });
  } catch {
    // 浏览器预览模式兜底
  }
});

async function handleMinimize() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().minimize();
  } catch {
    console.log("Minimize window (mock)");
  }
}

async function handleToggleMaximize() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    await appWindow.toggleMaximize();
    isMaximized.value = await appWindow.isMaximized();
  } catch {
    isMaximized.value = !isMaximized.value;
  }
}

async function handleClose() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  } catch {
    console.log("Close window (mock)");
  }
}

function navigateTo(path: string) {
  router.push(path);
}
</script>

<template>
  <div class="control-capsule-container">
    <div class="control-capsule glass-effect" :class="{ 'subpage-mode': !isHomePage }">
      <!-- 非首页模式下，展示完整返回与多页面快捷导航 -->
      <template v-if="!isHomePage">
        <button
          class="capsule-btn nav-btn home-btn"
          title="🏠 返回主舱"
          @click="navigateTo('/')"
        >
          <span>🏠</span>
        </button>

        <button
          class="capsule-btn nav-btn"
          :class="{ active: route.path === '/proxies' }"
          title="🚀 代理节点"
          @click="navigateTo('/proxies')"
        >
          <span>🚀</span>
        </button>

        <button
          class="capsule-btn nav-btn"
          :class="{ active: route.path === '/routing' }"
          title="🛠️ 分流配置"
          @click="navigateTo('/routing')"
        >
          <span>🛠️</span>
        </button>

        <button
          class="capsule-btn nav-btn"
          :class="{ active: route.path === '/audit' }"
          title="🔍 安全审计"
          @click="navigateTo('/audit')"
        >
          <span>🔍</span>
        </button>

        <button
          class="capsule-btn nav-btn"
          :class="{ active: route.path === '/speedtest' }"
          title="📊 智能测速"
          @click="navigateTo('/speedtest')"
        >
          <span>📊</span>
        </button>
      </template>

      <!-- 设置入口按钮 -->
      <button
        class="capsule-btn settings-btn"
        :class="{ active: route.path === '/settings' }"
        title="⚙️ 设置"
        @click="navigateTo('/settings')"
      >
        <span>⚙️</span>
      </button>

      <!-- 防误触物理分割线 -->
      <div class="capsule-divider" />

      <!-- 窗口控制按钮群 -->
      <div class="window-controls">
        <button class="capsule-btn minimize-btn" title="最小化" @click="handleMinimize">
          ─
        </button>

        <button
          class="capsule-btn maximize-btn"
          :title="isMaximized ? '还原' : '最大化'"
          @click="handleToggleMaximize"
        >
          {{ isMaximized ? '❐' : '⬜' }}
        </button>

        <button class="capsule-btn close-btn" title="关闭" @click="handleClose">
          ✕
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.control-capsule-container {
  position: absolute;
  top: 12px;
  right: 16px;
  z-index: 10000;
  -webkit-app-region: no-drag;
}

.control-capsule {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  background: rgba(18, 22, 34, 0.85);
  backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 20px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

.control-capsule:hover {
  border-color: rgba(0, 242, 254, 0.3);
  background: rgba(24, 30, 46, 0.92);
}

.capsule-divider {
  width: 1px;
  height: 14px;
  background: rgba(255, 255, 255, 0.12);
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
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: none;
  background: transparent;
  color: rgba(255, 255, 255, 0.7);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.capsule-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
  transform: scale(1.05);
}

.capsule-btn.active {
  background: rgba(0, 242, 254, 0.2);
  color: #00f2fe;
  box-shadow: 0 0 10px rgba(0, 242, 254, 0.3);
}

.close-btn:hover {
  background: #f87171;
  color: #fff;
}

.home-btn {
  background: rgba(255, 255, 255, 0.08);
}
</style>
