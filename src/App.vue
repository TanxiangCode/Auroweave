<script setup lang="ts">
/**
 * 根组件 App.vue
 * 作者: TanXiang
 *
 * 职责：
 * - 无边框窗口拖拽区域（data-tauri-drag-region）
 * - 初始化加载设置（主题、性能模式）
 * - 全局控制胶囊 (ControlCapsule) 挂载
 * - RouterView 承载五个主视图
 */
import { onMounted, ref } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { RouterView } from "vue-router";
import ControlCapsule from "@/components/chrome/ControlCapsule.vue";
import TrafficLights from "@/components/chrome/TrafficLights.vue";

const settingsStore = useSettingsStore();
const isMac = ref(false);

onMounted(async () => {
  isMac.value = navigator.userAgent.toLowerCase().includes("mac");
  await settingsStore.fetchSettings();
});
</script>

<template>
  <div class="app-shell" data-tauri-drag-region>
    <!-- 全局控制胶囊 (设置 + 窗口按钮) -->
    <ControlCapsule />

    <!-- macOS 专用红绿灯支持层 -->
    <TrafficLights v-if="isMac" />

    <!-- 主内容区 -->
    <main class="app-main">
      <RouterView v-slot="{ Component }">
        <Transition name="page" mode="out-in">
          <component :is="Component" />
        </Transition>
      </RouterView>
    </main>
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
  background: var(--border-strong);
  border-radius: var(--radius-full);
}
::-webkit-scrollbar-thumb:hover {
  background: var(--text-tertiary);
}
</style>

<style scoped>
/* ====================================================
   无边框窗口外壳
   ==================================================== */
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
  -webkit-app-region: drag; /* 顶层区域支持窗口拖拽 */
}

.app-main {
  flex: 1;
  overflow: hidden;
  position: relative;
  -webkit-app-region: no-drag; /* 内容区域解除拖拽拦截 */
}

/* ====================================================
   路由切换过渡动画
   ==================================================== */
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