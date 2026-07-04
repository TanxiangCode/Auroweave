<template>
  <div class="speed-chart-card">
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
    <div class="canvas-wrapper" ref="wrapperRef">
      <canvas ref="canvasRef"></canvas>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from "vue";
import { useConnectionStore } from "@/stores/connection.store";

const connectionStore = useConnectionStore();
const wrapperRef = ref<HTMLDivElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);

let animationFrameId: number | null = null;

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

  // 绘制下载曲线 (Cyan)
  drawCurve(
    ctx,
    points.map((p) => p.download),
    maxSpeed,
    width,
    height,
    stepX,
    "#00f2fe",
    "rgba(0, 242, 254, 0.15)"
  );

  // 绘制上传曲线 (Purple/Blue)
  drawCurve(
    ctx,
    points.map((p) => p.upload),
    maxSpeed,
    width,
    height,
    stepX,
    "#7f00ff",
    "rgba(127, 0, 255, 0.12)"
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

watch(() => connectionStore.speedHistory.length, () => {
  renderChart();
});

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
  background: var(--surface-card, rgba(255, 255, 255, 0.03));
  border: 1px solid var(--border-color, rgba(255, 255, 255, 0.08));
  border-radius: var(--radius-xl, 16px);
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  backdrop-filter: blur(12px);
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
  font-size: 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.85);
}

.pulse-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #00f2fe;
  box-shadow: 0 0 10px #00f2fe;
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
  font-size: 13px;
}

.indicator.download .arrow,
.indicator.download .val {
  color: #00f2fe;
  font-weight: 600;
}

.indicator.upload .arrow,
.indicator.upload .val {
  color: #a855f7;
  font-weight: 600;
}

.indicator .label {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
}

.canvas-wrapper {
  width: 100%;
  height: 140px;
  position: relative;
}

canvas {
  display: block;
}
</style>
