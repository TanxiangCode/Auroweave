<script setup lang="ts">
/**
 * 中央能量核
 * 作者: TanXiang
 *
 * 视觉绝对重心：旋转圆环 + 呼吸动效 + 连接/待命状态图标
 *
 * 动画效果参考小米充电动画：
 *   圆环大部分暗淡，一段亮色"彗星"光带沿环旋转。
 *
 * 分层结构（z-index 从低到高）：
 *   .energy-ring            — 容器 + 暗色环底 + boxShadow 光晕
 *     .energy-rotor         — conic-gradient 彗星旋转层（仅 active 时存在）
 *     .energy-rotor::before — 模糊光晕（跟随彗星，营造泛光效果）
 *     .energy-inner         — 静止内容层（图标 + 文字）
 */
import { computed } from "vue";
import { withDefaults } from "vue";
import { CORE_GLOW_THEMES } from "../utils/core-glow-themes";
import IdleTipsPanel from "./IdleTipsPanel.vue";

const props = withDefaults(
  defineProps<{
    proxyActive: boolean;
    coreStarting?: boolean;
    proxyMode: string;
    rotationDeg: number;
  }>(),
  {
    proxyActive: false,
    coreStarting: false,
    proxyMode: "rule",
    rotationDeg: 0,
  }
);

const emit = defineEmits<{
  toggle: [];
}>();

/** 当前主题（活跃 / 空闲 / 启动中） */
const currentTheme = computed(() => {
  if (props.coreStarting) return CORE_GLOW_THEMES["rule"]; // 启动中显示 rule 主题
  if (!props.proxyActive) return null;
  const mode = props.proxyMode as keyof typeof CORE_GLOW_THEMES;
  return CORE_GLOW_THEMES[mode] || CORE_GLOW_THEMES.rule;
});

/** 静止容器样式：暗色环底 + boxShadow 光晕 */
const ringStyle = computed(() => {
  if (!props.proxyActive) {
    return { background: "var(--energy-idle)", boxShadow: "none" };
  }
  return {
    background: currentTheme.value?.ring.ringBase ?? "transparent",
    boxShadow: currentTheme.value?.ring.boxShadow,
  };
});

/** 旋转层样式：conic-gradient 背景 */
const rotorStyle = computed(() => {
  if (!props.proxyActive) return {};
  return { background: currentTheme.value?.ring.background };
});

/** 文字样式 */
const textStyle = computed<{ color?: string; textShadow?: string }>(() => {
  if (!props.proxyActive) {
    return { color: "var(--text-secondary)", textShadow: "none" };
  }
  return currentTheme.value?.text || { color: "var(--accent-cyan-vivid)" };
});

/** 呼吸灯光晕颜色（配合 v-bind 实现动态关键帧） */
const breathingGlowColor = computed(() => {
  if (!props.proxyActive) return "rgba(255, 255, 255, 0.05)";
  return currentTheme.value?.breathing || "rgba(0, 242, 254, 0.4)";
});
</script>

<template>
  <div class="energy-wing" :class="{ 'full-center': !proxyActive }">
    <div
      class="energy-core"
      :class="{ connected: proxyActive }"
      @click="emit('toggle')"
      :title="proxyActive ? '网络已接管，点击安全释放并休眠' : '核心已待命，点击唤醒并接管流量'"
    >
      <div class="energy-ring" :style="ringStyle">
        <!-- 旋转背景环（conic-gradient 光效层） -->
        <div
          v-if="proxyActive"
          class="energy-rotor"
          :style="[rotorStyle, { transform: 'rotate(' + rotationDeg + 'deg)' }]"
        ></div>
        <!-- 静止内容层（图标 + 文字不随环旋转） -->
        <div class="energy-inner">
          <!-- 启动中状态 -->
          <template v-if="coreStarting">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="core-starting-icon" :style="{ color: textStyle.color }">
              <path d="M21 12a9 9 0 1 1-6.219-8.56"></path>
            </svg>
            <span class="energy-status-text starting-badge" :style="textStyle">启动中...</span>
          </template>

          <!-- 已连接状态 -->
          <template v-else-if="proxyActive">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="core-active-icon" :style="{ color: textStyle.color }">
              <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline>
            </svg>
            <span class="energy-status-text active-badge" :style="textStyle">CONNECTED</span>
          </template>
          <!-- 待机状态 -->
          <template v-else>
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="core-idle-icon">
              <path d="M18.36 6.64a9 9 0 1 1-12.73 0"></path>
              <line x1="12" y1="2" x2="12" y2="12"></line>
            </svg>
            <span class="energy-status-text idle">TAP TO CONNECT</span>
          </template>
        </div>
      </div>
    </div>

    <!-- 关闭代理时的极简状态说明 -->
    <IdleTipsPanel v-if="!proxyActive" />
  </div>
</template>

<style scoped>
.energy-wing {
  display: flex;
  justify-content: center;
  align-items: center;
  height: 260px;
}

.energy-core {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: transform var(--duration-fast) var(--ease-out);
}

.energy-core.connected {
  animation: breathing-core 3s infinite ease-in-out;
}

.energy-core:hover {
  transform: scale(1.02);
}

.energy-ring {
  position: relative;
  width: 240px;
  height: 240px;
  border-radius: 50%;
  padding: 4px;
  background: var(--energy-active);
  box-shadow: var(--shadow-glow-cyan);
  transition: background 0.6s ease-in-out, box-shadow 0.6s ease-in-out;
  overflow: hidden;
}

/* 旋转层：承载 conic-gradient，不带 transition 避免 360° 回弹 */
.energy-rotor {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  z-index: 0;
  pointer-events: none;
}

/* 旋转层彗星光晕：聚焦在亮色区域，营造头亮尾暗的泛光效果 */
.energy-rotor::before {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: 50%;
  background: inherit;
  filter: blur(20px);
  opacity: 0.7;
}

.energy-core:not(.connected) .energy-ring::before {
  opacity: 0;
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
  position: relative;
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
  z-index: 1;
}

.energy-status-text {
  font-size: var(--text-sm);
  font-weight: var(--weight-bold);
  letter-spacing: 2px;
  transition: color 0.6s ease-in-out, text-shadow 0.6s ease-in-out;
}

.energy-status-text.active-badge {
  font-size: var(--text-sm);
  letter-spacing: 2px;
  margin-top: 4px;
}

.core-starting-icon {
  width: 48px;
  height: 48px;
  margin-bottom: 4px;
  animation: core-spin 1.2s linear infinite;
}

@keyframes core-spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.energy-status-text.starting-badge {
  font-size: var(--text-sm);
  letter-spacing: 2px;
  margin-top: 4px;
  animation: pulse-opacity 1.5s infinite ease-in-out;
}

@keyframes pulse-opacity {
  0%, 100% { opacity: 0.6; }
  50% { opacity: 1; }
}

.core-active-icon {

  width: 48px;
  height: 48px;
  margin-bottom: 4px;
  opacity: 0.8;
  animation: pulse-line 3s infinite ease-in-out;
}

@keyframes pulse-line {
  0% { opacity: 0.4; transform: scaleY(0.95); }
  50% { opacity: 1; transform: scaleY(1.05); filter: drop-shadow(0 0 8px currentColor); }
  100% { opacity: 0.4; transform: scaleY(0.95); }
}

.energy-status-text.idle {
  color: var(--text-secondary);
  text-shadow: none;
  transition: color 0.3s ease;
}

.core-idle-icon {
  width: 48px;
  height: 48px;
  color: var(--text-secondary);
  transition: color 0.3s ease;
}

.energy-core:hover .core-idle-icon,
.energy-core:hover .energy-status-text.idle {
  color: var(--text-primary);
}

.energy-wing.full-center {
  flex-direction: column;
  height: auto;
  gap: 36px;
  justify-content: center;
  align-items: center;
}

@keyframes breathing-core {
  0% {
    transform: scale(1);
    filter: drop-shadow(0 0 8px v-bind(breathingGlowColor));
  }
  50% {
    transform: scale(1.025);
    filter: drop-shadow(0 0 20px v-bind(breathingGlowColor));
  }
  100% {
    transform: scale(1);
    filter: drop-shadow(0 0 8px v-bind(breathingGlowColor));
  }
}
</style>
