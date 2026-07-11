<template>
  <div class="speed-chart-card" :class="{ compact: compact }">
    <div class="chart-header">
      <div class="chart-title">
        <span class="pulse-dot"></span>
        实时网络流量趋势
      </div>
      <div class="speed-indicators">
        <div class="indicator download">
          <span class="arrow">↓</span>
          <span class="label">下载</span>
          <span class="val">{{ connectionStore.formatSpeed(connectionStore.rawDownloadSpeed) }}</span>
        </div>
        <div class="indicator upload">
          <span class="arrow">↑</span>
          <span class="label">上传</span>
          <span class="val">{{ connectionStore.formatSpeed(connectionStore.rawUploadSpeed) }}</span>
        </div>
      </div>
    </div>
    <div class="canvas-wrapper" :style="{ height: compact ? '80px' : '140px' }" ref="wrapperRef">
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

const getCssVar = (name: string, fallback: string) => {
  if (typeof window === "undefined") return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
};

const renderChart = () => {
  const canvas = canvasRef.value;
  const wrapper = wrapperRef.value;
  if (!canvas || !wrapper) return;

  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  const width = wrapper.clientWidth;
  const height = wrapper.clientHeight;

  canvas.width = width * dpr;
  canvas.height = height * dpr;
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;

  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, width, height);

  const points = connectionStore.speedHistory;
  if (points.length < 2) return;

  // 计算 Y 轴最大值（至少 100 KB/s）
  let maxSpeed = 1024 * 100;
  for (const p of points) {
    if (p.download > maxSpeed) maxSpeed = p.download;
    if (p.upload > maxSpeed) maxSpeed = p.upload;
  }

  const stepX = width / (points.length - 1);

  // 绘制网格背景线
  ctx.strokeStyle = "rgba(255, 255, 255, 0.04)";
  ctx.lineWidth = 1;
  for (let i = 1; i <= 3; i++) {
    const y = (height / 4) * i;
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(width, y);
    ctx.stroke();
  }

  const downloadColor = getCssVar("--accent-cyan-glow", "rgba(0, 242, 254, 0.8)");
  const downloadFill = "rgba(0, 242, 254, 0.15)";
  const uploadColor = getCssVar("--accent-purple", "#a855f7");
  const uploadFill = "rgba(168, 85, 247, 0.12)";

  // 绘制下载曲线 (Cyan)
  drawCurve(
    ctx,
    points.map((p) => p.download),
    maxSpeed,
    width,
    height,
    stepX,
    downloadColor,
    downloadFill
  );

  // 绘制上传曲线 (Purple)
  drawCurve(
    ctx,
    points.map((p) => p.upload),
    maxSpeed,
    width,
    height,
    stepX,
    uploadColor,
    uploadFill
  );
};

const drawCurve = (
  ctx: CanvasRenderingContext2D,
  data: number[],
  maxVal: number,
  width: number,
  height: number,
  stepX: number,
  lineColor: string,
  fillColor: string
) => {
  if (data.length < 2) return;

  ctx.beginPath();
  const getX = (i: number) => i * stepX;
  const getY = (v: number) => height - (v / maxVal) * (height - 20) - 10;

  ctx.moveTo(getX(0), getY(data[0]));

  for (let i = 1; i < data.length; i++) {
    const prevX = getX(i - 1);
    const prevY = getY(data[i - 1]);
    const currX = getX(i);
    const currY = getY(data[i]);
    const cpX = (prevX + currX) / 2;

    ctx.bezierCurveTo(cpX, prevY, cpX, currY, currX, currY);
  }

  // 描边
  ctx.strokeStyle = lineColor;
  ctx.lineWidth = 2;
  ctx.stroke();

  // 渐变填充
  ctx.lineTo(width, height);
  ctx.lineTo(0, height);
  ctx.closePath();

  const gradient = ctx.createLinearGradient(0, 0, 0, height);
  gradient.addColorStop(0, fillColor);
  gradient.addColorStop(1, "rgba(0, 0, 0, 0)");
  ctx.fillStyle = gradient;
  ctx.fill();
};

watch(
  () => connectionStore.speedHistory,
  () => {
    renderChart();
  },
  { deep: true }
);

onMounted(() => {
  renderChart();
  window.addEventListener("resize", renderChart);
});

onUnmounted(() => {
  window.removeEventListener("resize", renderChart);
  if (animationFrameId) cancelAnimationFrame(animationFrameId);
});
</script>

<style scoped>
.speed-chart-card {
  background: var(--layer-1);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-xl, 16px);
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  backdrop-filter: var(--blur-panel);
  box-shadow: var(--shadow-sm);
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
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
}

.pulse-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--accent-cyan);
  box-shadow: var(--shadow-glow-cyan);
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
  font-size: var(--text-sm);
}

.indicator.download .arrow,
.indicator.download .val {
  color: var(--accent-cyan);
  font-weight: var(--weight-bold);
}

.indicator.upload .arrow,
.indicator.upload .val {
  color: var(--accent-purple, #a855f7);
  font-weight: var(--weight-bold);
}

.indicator .label {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.canvas-wrapper {
  width: 100%;
  position: relative;
}

canvas {
  display: block;
}
</style>
