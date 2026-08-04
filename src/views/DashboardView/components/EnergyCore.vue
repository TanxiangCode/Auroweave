<script setup lang="ts">
/**
 * 中央能量核
 * 作者: TanXiang
 *
 * 视觉绝对重心：旋转圆环 + 呼吸动效 + 连接/待命状态图标
 */
import { computed } from "vue";
import { CORE_GLOW_THEMES } from "../utils/core-glow-themes";
import IdleTipsPanel from "./IdleTipsPanel.vue";

const props = defineProps<{
  proxyActive: boolean;
  proxyMode: string;
  rotationDeg: number;
}>();

const emit = defineEmits<{
  toggle: [];
}>();

/** 能量核光圈和文字阴影样式 */
const coreGlowStyle = computed(() => {
  if (!props.proxyActive) {
    return {
      ring: { background: "var(--energy-idle)", boxShadow: "none" },
      text: { color: "var(--text-secondary)", textShadow: "none" },
    };
  }
  const mode = props.proxyMode as keyof typeof CORE_GLOW_THEMES;
  return CORE_GLOW_THEMES[mode] || CORE_GLOW_THEMES.rule;
});

/** 呼吸灯光晕颜色（配合 v-bind 实现动态关键帧） */
const breathingGlowColor = computed(() => {
  if (!props.proxyActive) return "rgba(255, 255, 255, 0.05)";
  const mode = props.proxyMode as keyof typeof CORE_GLOW_THEMES;
  return CORE_GLOW_THEMES[mode]?.breathing || "rgba(0, 242, 254, 0.4)";
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
      <div
        class="energy-ring"
        :style="[
          { transform: proxyActive ? 'rotate(' + rotationDeg + 'deg)' : 'none' },
          coreGlowStyle.ring
        ]"
      >
        <div class="energy-inner">
          <template v-if="proxyActive">
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" class="core-active-icon" :style="{ color: coreGlowStyle.text.color }">
              <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"></polyline>
            </svg>
            <span class="energy-status-text active-badge">CONNECTED</span>
          </template>
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
  transition: background 0.6s ease-in-out, box-shadow 0.6s ease-in-out, transform var(--duration-normal) var(--ease-out);
  z-index: 1;
}

.energy-ring::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  border-radius: 50%;
  background: inherit;
  filter: blur(28px);
  opacity: 0.8;
  z-index: -1;
  transition: opacity 0.6s ease-in-out;
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
}

.energy-status-text {
  font-size: var(--text-sm);
  font-weight: var(--weight-bold);
  color: var(--accent-cyan);
  letter-spacing: 2px;
  text-shadow: var(--shadow-glow-cyan);
  transition: color 0.6s ease-in-out, text-shadow 0.6s ease-in-out;
}

.energy-status-text.active-badge {
  font-size: var(--text-sm);
  letter-spacing: 2px;
  margin-top: 4px;
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
