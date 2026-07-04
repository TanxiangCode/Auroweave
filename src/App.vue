<script setup lang="ts">
/**
 * 根组件 App.vue
 * 作者: TanXiang
 *
 * 职责：
 * - 无边框窗口拖拽区域（data-tauri-drag-region）
 * - 初始化加载设置（主题、性能模式）
 * - RouterView 承载五个主视图
 * - 全局控制胶囊挂载点
 */
import { onMounted } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { RouterView } from "vue-router";

const settingsStore = useSettingsStore();

onMounted(async () => {
  // 启动时加载设置（主题/性能模式会在 fetchSettings 内自动注入 DOM 属性）
  await settingsStore.fetchSettings();
});
</script>

<template>
  <div class="app-shell" data-tauri-drag-region>
    <!-- 全局控制胶囊（TODO 模块K：ControlCapsule 组件） -->
    <div class="control-capsule-placeholder" />

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
   全局基础样式（只有真正全局的规则才放这里）
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
  background: transparent; /* 透明窗口底色由操作系统提供 */
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

/* 禁用默认的文字选中（桌面客户端体验） */
body {
  user-select: none;
  cursor: default;
}

/* 输入框和可编辑区域恢复文字选中 */
input,
textarea,
[contenteditable] {
  user-select: text;
  cursor: text;
}

/* 自定义滚动条（深色主题） */
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
  /* 窗口边缘高光（暗色桌面下的轮廓感） */
  box-shadow: inset 0 0 0 1px var(--window-edge);
  border-radius: 12px; /* macOS 窗口圆角 */
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* 控制胶囊占位（右上角） */
.control-capsule-placeholder {
  position: absolute;
  top: var(--space-3);
  right: var(--space-3);
  width: 120px;
  height: 32px;
  z-index: 1000;
  /* TODO(模块K): 移除此占位，替换为 ControlCapsule 组件 */
}

.app-main {
  flex: 1;
  overflow: hidden;
  position: relative;
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