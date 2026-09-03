<script setup lang="ts">
/**
 * 分时流量柱状图
 * 作者: TanXiang
 *
 * 纯 SVG 绘制的高清发光柱状图，含日/月/年维度切换器
 */
import { computed } from "vue";
import { formatBytes } from "@/utils/format";
import type { ChartPoint } from "../hooks/useTrafficHistory";

const props = defineProps<{
  chartData: ChartPoint[];
  timeDimension: string;
}>();

const emit = defineEmits<{
  'update:timeDimension': [value: "day" | "month" | "year"];
}>();

// === 布局常量（与 SVG viewBox 0 0 800 240 保持一致）===
/** 绘图区左右边界 */
const CHART_LEFT = 45;
const CHART_RIGHT = 760;

/**
 * 统一的 x 轴布局：rect 与 text 必须使用同一 step 与 x 公式，避免错位。
 * step = (右边界 - 左边界) / 柱数；柱中心 x = 左边界 + (i + 0.5) * step。
 */
const layout = computed(() => {
  const count = Math.max(props.chartData.length, 1);
  const step = (CHART_RIGHT - CHART_LEFT) / count;
  const barWidth = Math.max(2, step * 0.6);
  return { step, barWidth };
});

/** 第 i 根柱的中心 x 坐标（rect 与 text 共用） */
function barCenterX(i: number): number {
  return CHART_LEFT + (i + 0.5) * layout.value.step;
}

/** x 轴刻度文本的抽稀：每隔约 6 个柱显示一个，最后一个必显 */
function isTickVisible(i: number): boolean {
  const count = props.chartData.length;
  if (count === 0) return false;
  const every = Math.max(1, Math.ceil(count / 6));
  return i % every === 0 || i === count - 1;
}
</script>

<template>
  <div class="chart-box glass-effect">
    <div class="chart-box-header">
      <h3 class="chart-box-title">
        {{ timeDimension === 'day' ? '近 24 小时' : timeDimension === 'month' ? '近 30 天' : '近 12 个月' }}流量对比趋势
      </h3>
      <div class="dimension-switcher">
        <button :class="{ active: timeDimension === 'day' }" @click="emit('update:timeDimension', 'day')">日</button>
        <button :class="{ active: timeDimension === 'month' }" @click="emit('update:timeDimension', 'month')">月</button>
        <button :class="{ active: timeDimension === 'year' }" @click="emit('update:timeDimension', 'year')">年</button>
      </div>
    </div>
    <div class="bar-chart-wrapper">
      <svg class="bar-chart-svg" viewBox="0 0 800 240">
        <defs>
          <linearGradient id="barGrad" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="var(--accent-cyan)" stop-opacity="0.8" />
            <stop offset="100%" stop-color="var(--accent-blue-glow)" stop-opacity="0.1" />
          </linearGradient>
        </defs>

        <line x1="40" y1="40" x2="760" y2="40" stroke="var(--border-subtle)" stroke-dasharray="4 4" />
        <line x1="40" y1="120" x2="760" y2="120" stroke="var(--border-subtle)" stroke-dasharray="4 4" />
        <line x1="40" y1="200" x2="760" y2="200" stroke="var(--border-strong)" />

        <g v-for="(bar, i) in chartData" :key="i">
          <rect
            :x="barCenterX(i) - layout.barWidth / 2"
            :y="200 - bar.heightPercent"
            :width="layout.barWidth"
            :height="bar.heightPercent"
            fill="url(#barGrad)"
            rx="3"
            class="bar-rect"
          >
            <title>{{ bar.label }} - 流量: {{ formatBytes(bar.bytes) }}</title>
          </rect>
        </g>

        <text
          v-for="(bar, i) in chartData"
          :key="i"
          :x="barCenterX(i)"
          y="220"
          class="svg-text"
          text-anchor="middle"
          :opacity="isTickVisible(i) ? 1 : 0"
        >
          {{ bar.label }}
        </text>
      </svg>
    </div>
  </div>
</template>

<style scoped>
.chart-box {
  display: flex;
  flex-direction: column;
  padding: 20px;
  gap: 16px;
}

.chart-box-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.chart-box-title {
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-weight: var(--weight-bold);
}

.dimension-switcher {
  display: flex;
  background: var(--layer-2);
  border-radius: var(--radius-sm);
  padding: 2px;
}

.dimension-switcher button {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 12px;
  padding: 4px 10px;
  border-radius: var(--radius-xs);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.dimension-switcher button:hover {
  color: var(--text-secondary);
}

.dimension-switcher button.active {
  background: var(--layer-3);
  color: var(--text-primary);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

.bar-chart-wrapper {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.bar-chart-svg {
  width: 100%;
  height: auto;
}

.bar-rect {
  transition: all var(--duration-fast);
  cursor: pointer;
}

.bar-rect:hover {
  fill: var(--accent-cyan);
  filter: drop-shadow(0 0 6px var(--accent-cyan));
}

.svg-text {
  fill: var(--text-tertiary);
  font-size: 10px;
  font-family: var(--font-mono, monospace);
}
</style>
