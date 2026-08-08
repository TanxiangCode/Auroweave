<script setup lang="ts">
/**
 * macOS 专用红绿灯控件
 * 作者: TanXiang
 *
 * 在无边框窗口 (decorations: false) 下渲染自定义红绿灯按钮，
 * 外观与 macOS 原生交通灯一致：红/黄/绿三色圆点，悬停时显示符号。
 */
import { ref, onMounted } from "vue";

const isMaximized = ref(false);

onMounted(async () => {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    isMaximized.value = await appWindow.isMaximized();
    appWindow.onResized(async () => {
      isMaximized.value = await appWindow.isMaximized();
    });
  } catch {
    // 浏览器预览模式跳过
  }
});

async function handleClose() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  } catch {
    // mock
  }
}

async function handleMinimize() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().minimize();
  } catch {
    // mock
  }
}

async function handleToggleMaximize() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().toggleMaximize();
  } catch {
    // mock
  }
}
</script>

<template>
  <div class="traffic-lights">
    <button class="light light-close" title="关闭" @click="handleClose">
      <svg class="light-icon" width="8" height="8" viewBox="0 0 8 8" fill="none">
        <path d="M1.5 1.5 L6.5 6.5 M6.5 1.5 L1.5 6.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
      </svg>
    </button>
    <button class="light light-minimize" title="最小化" @click="handleMinimize">
      <svg class="light-icon" width="8" height="8" viewBox="0 0 8 8" fill="none">
        <path d="M1.5 4 L6.5 4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
      </svg>
    </button>
    <button class="light light-maximize" :title="isMaximized ? '还原' : '最大化'" @click="handleToggleMaximize">
      <svg v-if="!isMaximized" class="light-icon" width="8" height="8" viewBox="0 0 8 8" fill="none">
        <path d="M2 2 L6 2 L6 6 L2 6 Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
      </svg>
      <svg v-else class="light-icon" width="8" height="8" viewBox="0 0 8 8" fill="none">
        <path d="M2.5 5.5 L2.5 2.5 L5.5 2.5 M5.5 5.5 L5.5 3 M3 5.5 L5.5 5.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>
  </div>
</template>

<style scoped>
.traffic-lights {
  position: absolute;
  top: 13px;
  left: 13px;
  display: flex;
  align-items: center;
  gap: 8px;
  z-index: 10000;
  -webkit-app-region: no-drag;
  app-region: no-drag;
}

.light {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: none;
  padding: 0;
  cursor: pointer;
  -webkit-app-region: no-drag;
  app-region: no-drag;
  transition: filter 0.15s ease;
}

.light:hover {
  filter: brightness(1.15);
}

/* 图标默认隐藏，悬停整组时显示 */
.light-icon {
  opacity: 0;
  color: rgba(0, 0, 0, 0.55);
  transition: opacity 0.12s ease;
}

.traffic-lights:hover .light-icon {
  opacity: 1;
}

.light-close {
  background: #ff5f57;
}

.light-minimize {
  background: #febc2e;
}

.light-maximize {
  background: #28c840;
}
</style>
