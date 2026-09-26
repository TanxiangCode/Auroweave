<template>
  <div class="speed-chart-card" :class="{ compact: compact }">
    <div class="chart-header">
      <div class="chart-title">
        <span class="pulse-dot"></span>
        <span>实时双向流量监控</span>
      </div>
      <div class="speed-indicators">
        <div class="indicator download">
          <span class="dot-badge blue"></span>
          <span class="label">下载</span>
          <span class="val">{{ connectionStore.formatSpeed(connectionStore.rawDownloadSpeed) }}</span>
        </div>
        <div class="indicator upload">
          <span class="dot-badge red"></span>
          <span class="label">上传</span>
          <span class="val">{{ connectionStore.formatSpeed(connectionStore.rawUploadSpeed) }}</span>
        </div>
      </div>
    </div>
    <div class="canvas-wrapper" :class="{ 'compact-height': compact }" ref="wrapperRef">
      <canvas ref="canvasRef"></canvas>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from "vue";
import { useConnectionStore } from "@/stores/connection.store";

withDefaults(
  defineProps<{
    compact?: boolean;
  }>(),
  {
    compact: false,
  }
);

const connectionStore = useConnectionStore();
const wrapperRef = ref<HTMLDivElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);

let animationFrameId: number | null = null;

// Canvas 2D 的 strokeStyle/fillStyle 不支持 CSS 变量：赋值 "var(--x)" 会被静默忽略，
// 保留上一次的颜色（初始为 #000 黑）。必须先用 getComputedStyle 取出真实色值再赋值。
// 主题切换会改变 token 定义，故缓存需在 data-theme 变化时失效。
let themeObserver: MutationObserver | null = null;
const cssVarCache = new Map<string, string>();

/** 读取当前主题下 CSS 变量的真实色值（带缓存，主题切换时清空） */
function cssVar(name: string, fallback: string): string {
  const hit = cssVarCache.get(name);
  if (hit) return hit;
  const v = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  const resolved = v || fallback;
  cssVarCache.set(name, resolved);
  return resolved;
}

/** 给 token 色值套上透明度，产出 canvas 可用的 rgba()。
 *  token 可能是 #rgb / #rrggbb / rgb() / rgba()，统一转换。 */
function withAlpha(color: string, alpha: number): string {
  const c = color.trim();
  let r = 0, g = 0, b = 0;
  if (c.startsWith("#")) {
    const hex = c.slice(1);
    const full = hex.length === 3 || hex.length === 4
      ? hex.split("").map((ch) => ch + ch).join("")
      : hex.slice(0, 6);
    r = parseInt(full.slice(0, 2), 16);
    g = parseInt(full.slice(2, 4), 16);
    b = parseInt(full.slice(4, 6), 16);
  } else {
    const m = c.match(/[\d.]+/g);
    if (m) { r = +m[0]; g = +m[1]; b = +m[2]; }
  }
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

// 平滑量程阻尼值
let currentMaxDown = 1024 * 100;
let currentMaxUp = 1024 * 50;

const renderChart = () => {
  const canvas = canvasRef.value;
  const wrapper = wrapperRef.value;
  if (!canvas || !wrapper) return;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  const width = wrapper.clientWidth;
  const height = wrapper.clientHeight;

  if (width === 0 || height === 0) return;

  canvas.width = width * dpr;
  canvas.height = height * dpr;
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;

  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, width, height);

  // 获取并补齐 60 个采样点，确保折线平稳铺满画布
  const rawPoints = connectionStore.speedHistory || [];
  const TOTAL_POINTS = 60;
  const points: { download: number; upload: number }[] = [];

  const padCount = Math.max(0, TOTAL_POINTS - rawPoints.length);
  for (let i = 0; i < padCount; i++) {
    points.push({ download: 0, upload: 0 });
  }
  for (let i = Math.max(0, rawPoints.length - TOTAL_POINTS); i < rawPoints.length; i++) {
    points.push({
      download: rawPoints[i].download,
      upload: rawPoints[i].upload,
    });
  }

  // 1. 计算当前采样周期内的峰值
  let targetMaxDown = 1024 * 50; // 默认下限 50 KB/s
  let targetMaxUp = 1024 * 20;   // 默认下限 20 KB/s
  for (const p of points) {
    if (p.download > targetMaxDown) targetMaxDown = p.download;
    if (p.upload > targetMaxUp) targetMaxUp = p.upload;
  }

  // 缓动平滑过度最大量程（避免突变跳跃）
  currentMaxDown = currentMaxDown * 0.8 + targetMaxDown * 0.2;
  currentMaxUp = currentMaxUp * 0.8 + targetMaxUp * 0.2;

  // 2. 几何布局：中轴线设定在 72% 高度处（上方 72% 下载，下方 28% 上传）
  const baselineY = Math.round(height * 0.72);
  const topGuideY = 10;
  const stepX = width / (TOTAL_POINTS - 1);

  // 3. 绘制顶部最大刻度参考线 (Top Reference Line)
  ctx.beginPath();
  ctx.strokeStyle = cssVar("--border-strong", "rgba(0, 0, 0, 0.15)");
  ctx.lineWidth = 1;
  ctx.moveTo(0, topGuideY);
  ctx.lineTo(width, topGuideY);
  ctx.stroke();

  // 4. 绘制下载流量折线与填充区 (上方波峰，亮青色)
  ctx.beginPath();
  ctx.moveTo(0, baselineY);

  for (let i = 0; i < points.length; i++) {
    const x = i * stepX;
    const ratio = Math.min(1, points[i].download / Math.max(currentMaxDown, 1024));
    const y = baselineY - ratio * (baselineY - topGuideY - 4);
    ctx.lineTo(x, y);
  }

  // 闭合到中轴线
  ctx.lineTo(width, baselineY);
  ctx.lineTo(0, baselineY);
  ctx.closePath();

  // 下载填充渐变色（与图例/描边同源，跟随主题）
  const downColor = cssVar("--accent-blue", "rgb(0, 127, 249)");
  const downFillGrad = ctx.createLinearGradient(0, topGuideY, 0, baselineY);
  downFillGrad.addColorStop(0, withAlpha(downColor, 0.55));
  downFillGrad.addColorStop(0.5, withAlpha(downColor, 0.45));
  downFillGrad.addColorStop(1, withAlpha(downColor, 0.2));
  ctx.fillStyle = downFillGrad;
  ctx.fill();

  // 下载折线描边 (下载蓝 rgb(0,127,249))
  ctx.beginPath();
  for (let i = 0; i < points.length; i++) {
    const x = i * stepX;
    const ratio = Math.min(1, points[i].download / Math.max(currentMaxDown, 1024));
    const y = baselineY - ratio * (baselineY - topGuideY - 4);
    if (i === 0) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
  }
  ctx.strokeStyle = downColor;
  ctx.lineWidth = 2;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.stroke();

  // 5. 绘制上传流量折线与填充区 (下方倒波峰，亮珊瑚红)
  ctx.beginPath();
  ctx.moveTo(0, baselineY);

  for (let i = 0; i < points.length; i++) {
    const x = i * stepX;
    const ratio = Math.min(1, points[i].upload / Math.max(currentMaxUp, 1024));
    const y = baselineY + ratio * (height - baselineY - 6);
    ctx.lineTo(x, y);
  }

  // 闭合到中轴线
  ctx.lineTo(width, baselineY);
  ctx.lineTo(0, baselineY);
  ctx.closePath();

  // 上传填充渐变色（与图例/描边同源，跟随主题）
  const upColor = cssVar("--status-danger", "rgb(254, 49, 56)");
  const upFillGrad = ctx.createLinearGradient(0, baselineY, 0, height);
  upFillGrad.addColorStop(0, withAlpha(upColor, 0.55));
  upFillGrad.addColorStop(0.6, withAlpha(upColor, 0.4));
  upFillGrad.addColorStop(1, withAlpha(upColor, 0.2));
  ctx.fillStyle = upFillGrad;
  ctx.fill();

  // 上传折线描边 (上传红 rgb(254,49,56))
  ctx.beginPath();
  for (let i = 0; i < points.length; i++) {
    const x = i * stepX;
    const ratio = Math.min(1, points[i].upload / Math.max(currentMaxUp, 1024));
    const y = baselineY + ratio * (height - baselineY - 6);
    if (i === 0) {
      ctx.moveTo(x, y);
    } else {
      ctx.lineTo(x, y);
    }
  }
  ctx.strokeStyle = upColor;
  ctx.lineWidth = 2;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.stroke();

  // 6. 绘制中轴线 (Baseline Divider)
  ctx.beginPath();
  ctx.strokeStyle = cssVar("--text-tertiary", "rgba(0, 0, 0, 0.4)");
  ctx.lineWidth = 1;
  ctx.moveTo(0, baselineY);
  ctx.lineTo(width, baselineY);
  ctx.stroke();
};

watch(
  () => [connectionStore.rawDownloadSpeed, connectionStore.rawUploadSpeed, connectionStore.speedHistory.length],
  () => {
    renderChart();
  }
);

onMounted(() => {
  renderChart();
  window.addEventListener("resize", renderChart);

  // 主题切换后 token 值已变，清缓存并重绘，否则图表仍用上一主题的线条颜色
  themeObserver = new MutationObserver(() => {
    cssVarCache.clear();
    renderChart();
  });
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme", "class"],
  });
});

onUnmounted(() => {
  window.removeEventListener("resize", renderChart);
  if (animationFrameId) cancelAnimationFrame(animationFrameId);
  themeObserver?.disconnect();
  themeObserver = null;
});
</script>

<style scoped>
.speed-chart-card {
  background: var(--layer-1);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-xl, 18px);
  padding: 14px 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  backdrop-filter: var(--blur-panel);
  box-shadow: var(--shadow-md);
  position: relative;
  overflow: hidden;
}

.speed-chart-card.compact {
  background: transparent;
  border: none;
  padding: 0;
  backdrop-filter: none;
  box-shadow: none;
  gap: 6px;
}

.speed-chart-card.compact .chart-title {
  display: none;
}

.chart-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.chart-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--text-xs, 12px);
  font-weight: var(--weight-semibold, 600);
  color: var(--text-secondary);
  letter-spacing: 0.3px;
}

.pulse-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent-blue);
  box-shadow: 0 0 8px color-mix(in srgb, var(--accent-blue) 80%, transparent);
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0% { transform: scale(0.95); opacity: 0.8; }
  50% { transform: scale(1.3); opacity: 1; }
  100% { transform: scale(0.95); opacity: 0.8; }
}

.speed-indicators {
  display: flex;
  gap: 16px;
}

.indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-xs, 12px);
  font-family: var(--font-mono, monospace);
}

.dot-badge {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.dot-badge.blue {
  background: var(--accent-blue);
  box-shadow: 0 0 6px color-mix(in srgb, var(--accent-blue) 70%, transparent);
}

.dot-badge.red {
  background: var(--status-danger);
  box-shadow: 0 0 6px color-mix(in srgb, var(--status-danger) 70%, transparent);
}

.indicator.download .val {
  color: var(--accent-blue);
  font-weight: var(--weight-bold, 700);
}

.indicator.upload .val {
  color: var(--status-danger);
  font-weight: var(--weight-bold, 700);
}

.indicator .label {
  color: var(--text-tertiary);
  font-size: 11px;
}

.canvas-wrapper {
  width: 100%;
  flex: 1;
  min-height: 80px;
  position: relative;
  border-radius: 8px;
  overflow: hidden;
  background: var(--surface-inset);
}

.canvas-wrapper.compact-height {
  flex: initial;
  height: 90px;
}

canvas {
  display: block;
  width: 100%;
  height: 100%;
}
</style>
