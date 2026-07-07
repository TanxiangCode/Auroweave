<script setup lang="ts">
/**
 * 全局控制胶囊 (Control Capsule)
 * 作者: TanXiang
 *
 * 视觉设计：
 * - 紧凑超精致毛玻璃浮层
 * - 遵守防误触设计：设置与关闭按钮物理隔离 > 60px
 * - macOS 环境下隐藏右侧 ─/⬜/✕ 按钮 (由 macOS 原生红绿灯接管)
 */
import { ref, onMounted } from "vue";
import { useRoute, useRouter } from "vue-router";
import SvgIcon from "@/components/common/SvgIcon.vue";
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
  } catch {
    // 浏览器预览模式兜底
  }
});

async function handleResetSize() {
  try {
    const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    await appWindow.setSize(new LogicalSize(WINDOW_DEFAULT_WIDTH, WINDOW_DEFAULT_HEIGHT));
    await appWindow.center();
  } catch {
    console.log(`Reset size to: ${WINDOW_DEFAULT_WIDTH}x${WINDOW_DEFAULT_HEIGHT}`);
  }
}

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
    <!-- 1. ⚙️ 设置按钮 (在设置页内高亮) -->
    <button
      class="capsule-btn settings-btn"
      :class="{ active: route.path === '/settings' }"
      title="设置"
      @click="handleSettingsClick"
    >
      <SvgIcon name="settings" :size="14" />
    </button>

    <!-- 2. 非 macOS 平台下的窗口控制按钮群 (设置与关闭分割间距遵循 SETTINGS_CLOSE_MIN_GAP_PX 防误触) -->
    <template v-if="!isMac">
      <div
        class="capsule-divider"
        :style="{ margin: `0 ${Math.max(6, SETTINGS_CLOSE_MIN_GAP_PX / 10)}px` }"
      />

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
    </template>
  </div>
</template>

<style scoped>
.control-capsule {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  box-shadow: var(--shadow-sm);
  transition: all var(--duration-normal) var(--ease-out);
}

.control-capsule:hover {
  border-color: var(--border-accent);
}

.capsule-divider {
  width: 1px;
  height: 14px;
  background: var(--border-subtle);
  margin: 0 6px;
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
}

.capsule-btn:hover {
  background: var(--border-strong);
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
