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
  ctx.strokeStyle = "rgba(255, 255, 255, 0.18)";
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

  // 下载填充渐变色（与图例/描边同源的科技蓝）
  const downFillGrad = ctx.createLinearGradient(0, topGuideY, 0, baselineY);
  downFillGrad.addColorStop(0, "rgba(0, 127, 249, 0.55)");
  downFillGrad.addColorStop(0.5, "rgba(0, 100, 200, 0.45)");
  downFillGrad.addColorStop(1, "rgba(10, 60, 130, 0.55)");
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
  ctx.strokeStyle = "rgb(0, 127, 249)";
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

  // 上传填充渐变色（与图例/描边同源的上传红）
  const upFillGrad = ctx.createLinearGradient(0, baselineY, 0, height);
  upFillGrad.addColorStop(0, "rgba(180, 20, 30, 0.55)");
  upFillGrad.addColorStop(0.6, "rgba(220, 30, 40, 0.4)");
  upFillGrad.addColorStop(1, "rgba(140, 15, 25, 0.5)");
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
  ctx.strokeStyle = "rgb(254, 49, 56)";
  ctx.lineWidth = 2;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.stroke();

  // 6. 绘制中轴线 (Baseline Divider)
  ctx.beginPath();
  ctx.strokeStyle = "rgba(255, 255, 255, 0.25)";
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
});

onUnmounted(() => {
  window.removeEventListener("resize", renderChart);
  if (animationFrameId) cancelAnimationFrame(animationFrameId);
});
</script>

<style scoped>
.speed-chart-card {
  background: #14161f;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-xl, 18px);
  padding: 14px 18px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  backdrop-filter: var(--blur-panel);
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.35);
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
  color: rgba(255, 255, 255, 0.6);
  letter-spacing: 0.3px;
}

.pulse-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: rgb(0, 127, 249);
  box-shadow: 0 0 8px rgba(0, 127, 249, 0.8);
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
  background: rgb(0, 127, 249);
  box-shadow: 0 0 6px rgba(0, 127, 249, 0.7);
}

.dot-badge.red {
  background: rgb(254, 49, 56);
  box-shadow: 0 0 6px rgba(254, 49, 56, 0.7);
}

.indicator.download .val {
  color: rgb(0, 127, 249);
  font-weight: var(--weight-bold, 700);
}

.indicator.upload .val {
  color: rgb(254, 49, 56);
  font-weight: var(--weight-bold, 700);
}

.indicator .label {
  color: rgba(255, 255, 255, 0.4);
  font-size: 11px;
}

.canvas-wrapper {
  width: 100%;
  flex: 1;
  min-height: 80px;
  position: relative;
  border-radius: 8px;
  overflow: hidden;
  background: #10121a;
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
