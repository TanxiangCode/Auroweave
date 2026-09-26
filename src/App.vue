<script setup lang="ts">
/**
 * 根组件 App.vue
 * 作者: TanXiang
 */
import { onMounted, ref, computed } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useCoreUpdateStore } from "@/stores/coreUpdate.store";
import { RouterView, useRoute, useRouter } from "vue-router";
import ControlCapsule from "@/components/chrome/ControlCapsule.vue";
import TrafficLights from "@/components/chrome/TrafficLights.vue";
import SvgIcon from "@/components/common/SvgIcon.vue";
import CommandPalette from "@/components/command-palette/CommandPalette.vue";
import Toast from "@/components/common/Toast.vue";
import ConfirmDialog from "@/components/common/ConfirmDialog.vue";
import { WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT } from "@/constants";
import { useGlobalHotkey } from "@/composables/useGlobalHotkey";
import { useConfirm } from "@/composables/useConfirm";

const settingsStore = useSettingsStore();
const coreUpdateStore = useCoreUpdateStore();
const route = useRoute();
const router = useRouter();
const isMac = ref(false);

/** 命令框组件引用：全局热键触发时呼出 */
const paletteRef = ref<InstanceType<typeof CommandPalette> | null>(null);

const showBack = computed(() => {
  return route.path !== "/" && route.path !== "/dashboard";
});

const routeTitle = computed(() => {
  return (route.meta.title as string) || "Auroweave";
});

function goBack() {
  router.back();
}

/**
 * 全局热键回调：确保窗口可见并置前，然后呼出命令框。
 * 窗口可能被隐藏到托盘（minimize_to_tray / start_minimized），必须先恢复。
 */
async function onGlobalHotkeyTriggered() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    await appWindow.show();
    await appWindow.setFocus();
  } catch {
    // 浏览器预览模式跳过窗口操作
  }
  paletteRef.value?.open();
}

// 注册系统级全局热键（设置加载完成后按 command_palette_hotkey 生效）
useGlobalHotkey(onGlobalHotkeyTriggered);

// 全局确认弹窗单例（替换原生 confirm）
const confirm = useConfirm();

onMounted(async () => {
  isMac.value = navigator.userAgent.toLowerCase().includes("mac");
  // 内核升级进度监听必须常驻：设置页高级面板由 v-else-if 按需挂载，
  // 若等面板挂载才监听，用户在别处触发的升级事件会被整段丢弃。
  // 同时回查后端进度，覆盖"升级中整页重载"的场景。
  void coreUpdateStore.init();
  await settingsStore.fetchSettings();

  try {
    const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    await appWindow.setMinSize(new LogicalSize(WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT));
  } catch {
    // 浏览器预览模式跳过
  }
});
</script>

<template>
  <div class="app-shell">
    <!-- 全局顶栏 (Tauri 拖拽区域) -->
    <header class="app-topbar" :class="{ 'is-mac': isMac }" data-tauri-drag-region>
      <div class="topbar-left">
        <button v-if="showBack" class="btn-back-nav" title="返回" @click="goBack">
          <SvgIcon name="back" :size="14" />
        </button>
        <span class="topbar-title">{{ routeTitle }}</span>
      </div>
      <div class="topbar-right" @mousedown.stop>
        <!-- 全局控制胶囊 (设置 + 窗口按钮) -->
        <ControlCapsule />
      </div>
    </header>

    <!-- macOS 专用红绿灯支持层 -->
    <TrafficLights v-if="isMac" />

    <!-- 主内容区 -->
    <main class="app-main">
      <RouterView v-slot="{ Component }">
        <Transition name="page" mode="out-in">
          <!-- KeepAlive 按 name 匹配 include 时依赖各视图显式声明组件名，
               故采用 :max 限制缓存数量上限（视图组件不足 8 个，效果等同全缓存但封顶内存） -->
          <KeepAlive :max="8">
            <component :is="Component" />
          </KeepAlive>
        </Transition>
      </RouterView>
    </main>

    <!-- 全局 Spotlight 快捷命令框 -->
    <CommandPalette ref="paletteRef" />

    <!-- 全局消息 Toast 提示框 -->
    <Toast />

    <!-- 全局危险操作确认弹窗（useConfirm 单例驱动） -->
    <ConfirmDialog
      :visible="confirm.state.visible.value"
      :title="confirm.state.title.value"
      :message="confirm.state.message.value"
      :confirm-text="confirm.state.confirmText.value"
      :level="confirm.state.level.value"
      @confirm="confirm.settle(true)"
      @cancel="confirm.settle(false)"
    />
  </div>
</template>

<style>
/* ====================================================
   全局基础样式
   ==================================================== */

*,
*::before,
*::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html,
body,
#app {
  width: 100%;
  height: 100%;
  overflow: hidden;
  font-family: var(--font-sans);
  font-size: var(--text-base);
  color: var(--text-primary);
  background: transparent;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

/* 桌面客户端禁止误选文本 */
body {
  user-select: none;
  cursor: default;
}

/* 可编辑元素恢复选中 */
input,
textarea,
[contenteditable] {
  user-select: text;
  cursor: text;
  -webkit-app-region: no-drag;
}

button, a {
  -webkit-app-region: no-drag;
}

::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}
::-webkit-scrollbar-track {
  background: transparent;
}
::-webkit-scrollbar-thumb {
  background: var(--surface-hover);
  border-radius: var(--radius-full);
}
::-webkit-scrollbar-thumb:hover {
  background: var(--text-tertiary);
}

/* ---- 全局复选框 (Switch) 样式微调，使其在未选中时清晰可见 ---- */
input[type="checkbox"].switch {
  -webkit-appearance: none;
  appearance: none;
  width: 36px;
  height: 20px;
  border-radius: var(--radius-full);
  background: var(--layer-3, var(--border-normal));
  border: 1.5px solid var(--border-strong, var(--text-tertiary));
  position: relative;
  outline: none;
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

input[type="checkbox"].switch::before {
  content: "";
  position: absolute;
  top: 2px;
  left: 2px;
  width: 13px;
  height: 13px;
  border-radius: 50%;
  background: var(--text-secondary);
  transition: all var(--duration-fast) var(--ease-out);
}

input[type="checkbox"].switch:checked {
  background: var(--accent-cyan-glow);
  border-color: var(--accent-cyan-vivid);
}

input[type="checkbox"].switch:checked::before {
  left: 18px;
  background: var(--accent-cyan-vivid);
  box-shadow: 0 0 6px var(--accent-cyan-glow);
}
</style>

<style scoped>
.app-shell {
  position: relative;
  width: 100vw;
  height: 100vh;
  background: var(--layer-0);
  box-shadow: inset 0 0 0 1px var(--window-edge);
  border-radius: 12px;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.app-topbar {
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border-bottom: 1px solid var(--border-subtle);
  z-index: 9999;
  -webkit-app-region: drag; /* 解决问题1：拖拽生效 */
}

.app-topbar.is-mac {
  padding-left: 115px; /* 避开 macOS 原生左侧红绿灯与设置齿轮 */
}

.topbar-left {
  display: flex;
  align-items: center;
  gap: 12px;
  -webkit-app-region: no-drag;
}

.btn-back-nav {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 1px solid var(--border-subtle);
  background: var(--surface-raised);
  color: var(--text-primary);
  font-size: 16px;
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-back-nav:hover {
  background: var(--surface-hover);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
  transform: scale(1.05);
}

.topbar-title {
  font-size: var(--text-base);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.topbar-right {
  display: flex;
  align-items: center;
  -webkit-app-region: no-drag;
}

.app-main {
  flex: 1;
  overflow: hidden;
  position: relative;
  -webkit-app-region: no-drag;
}

/* 路由切换过渡动画 */
.page-enter-active,
.page-leave-active {
  transition:
    opacity var(--duration-normal) var(--ease-out),
    transform var(--duration-normal) var(--ease-out);
}

.page-enter-from {
  opacity: 0;
  transform: scale(0.97);
}

.page-leave-to {
  opacity: 0;
  transform: scale(1.02);
}
</style>