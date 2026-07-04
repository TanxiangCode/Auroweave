<script setup lang="ts">
/**
 * 全局控制胶囊 (Control Capsule)
 * 作者: TanXiang
 *
 * 职责：
 * - 窗口控制：最小化 ─、最大化/还原 ⬜、关闭 ✕
 * - 设置入口：⚙️
 * - 遵守防误触设计：⚙️ 与 ✕ 物理距离 > 60px
 * - 在非 macOS 平台显示完整胶囊，macOS 下通过 TrafficLights 显示
 */
import { ref, onMounted } from "vue";
import { useRouter, useRoute } from "vue-router";

const router = useRouter();
const route = useRoute();

const isMaximized = ref(false);
const isMac = ref(false);

// 检测是否在 macOS 平台
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
    // 浏览器预览模式下无 Tauri API
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
    console.log("Toggle maximize window (mock)");
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

function handleSettingsClick() {
  if (route.path === "/settings") {
    router.back();
  } else {
    router.push("/settings");
  }
}
</script>

<template>
  <div class="control-capsule-container">
    <!-- 控制胶囊 (非 macOS 或 统一通用窗口控制) -->
    <div class="control-capsule" :class="{ 'in-settings': route.path === '/settings' }">
      <!-- 设置/返回按钮 -->
      <button
        class="capsule-btn settings-btn"
        :title="route.path === '/settings' ? '🔙 返回主舱' : '⚙️ 设置'"
        @click="handleSettingsClick"
      >
        <span v-if="route.path === '/settings'">🔙</span>
        <span v-else>⚙️</span>
      </button>

      <!-- 防误触物理分割线与间距 (设置与关闭隔离 > 60px) -->
      <div class="capsule-divider" />

      <!-- 窗口控制按钮 -->
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
  top: var(--space-3);
  right: var(--space-4);
  z-index: 10000;
  /* 确保按钮可点击 */
  -webkit-app-region: no-drag;
}

.control-capsule {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 4px 10px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  box-shadow: var(--shadow-sm);
  transition: all var(--duration-fast);
}

.control-capsule:hover {
  border-color: var(--border-strong);
  background: var(--layer-2);
}

.capsule-divider {
  width: 1px;
  height: 14px;
  background: var(--border-normal);
  margin: 0 var(--space-2);
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
  font-size: 12px;
  cursor: pointer;
  transition: all var(--duration-fast);
}

.capsule-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-primary);
}

.close-btn:hover {
  background: var(--accent-red);
  color: #fff;
}

.settings-btn {
  font-size: 14px;
}
</style>
