<script setup lang="ts">
/**
 * macOS 专用红绿灯控件安全间距层
 * 作者: TanXiang
 *
 * 保留 macOS 原生红绿灯样式，右侧提供安全间距与设置齿轮图标
 */
import { useRouter, useRoute } from "vue-router";

const router = useRouter();
const route = useRoute();

function handleSettingsClick() {
  if (route.path === "/settings") {
    router.back();
  } else {
    router.push("/settings");
  }
}
</script>

<template>
  <div class="traffic-lights-wrapper">
    <!-- 原生红绿灯安全间距占位 (80px) -->
    <div class="native-lights-placeholder" />

    <!-- 紧跟右侧的设置齿轮按钮 -->
    <button class="mac-settings-btn" title="⚙️ 设置" @click="handleSettingsClick">
      ⚙️
    </button>
  </div>
</template>

<style scoped>
.traffic-lights-wrapper {
  position: absolute;
  top: 14px;
  left: 14px;
  display: flex;
  align-items: center;
  gap: 12px;
  z-index: 10000;
  -webkit-app-region: no-drag;
}

.native-lights-placeholder {
  width: 70px;
  height: 20px;
}

.mac-settings-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: none;
  background: var(--layer-1);
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: all var(--duration-fast);
}

.mac-settings-btn:hover {
  background: var(--layer-2);
  color: var(--text-primary);
}
</style>
