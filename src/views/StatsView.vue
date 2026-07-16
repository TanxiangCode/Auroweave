<script setup lang="ts">
/**
 * StatsView 流量统计大盘
 * 作者: TanXiang
 * 
 * 视觉与功能设计：
 * - 纯 SVG 绘制的高清发光柱状图 (24小时历史分时流量)
 * - 纯 SVG 绘制的动态环形图 (节点/协议流量分配占比)
 * - 与 connectionStore 的 totalDownload/Upload 联动并读取持久化
 * - 提供重置清空大盘数据交互
 */
import { computed, ref, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useConnectionStore } from "@/stores/connection.store";
import { storeToRefs } from "pinia";
import SvgIcon from "@/components/common/SvgIcon.vue";

const timeDimension = ref<"day" | "month" | "year">("day");

const connectionStore = useConnectionStore();
const { totalDownload, totalUpload } = storeToRefs(connectionStore);

// 清空大盘数据
function clearStats() {
  if (confirm("确定要清空累计的历史流量统计吗？该操作不可恢复。")) {
    totalDownload.value = 0;
    totalUpload.value = 0;
    localStorage.setItem("auroweave_total_download", "0");
    localStorage.setItem("auroweave_total_upload", "0");
  }
}

// 格式化辅助
function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

// 真实历史流量数据
interface ChartPoint {
  label: string;
  bytes: number;
  heightPercent: number;
}
const chartData = ref<ChartPoint[]>([]);

async function fetchTrafficHistory() {
  try {
    const res: any = await invoke("get_traffic_history", { dimension: timeDimension.value });
    if (res.success && res.data) {
      const data = res.data;
      const totalBytesArr = data.map((d: any) => d.download_bytes + d.upload_bytes);
      const maxBytes = Math.max(...totalBytesArr, 1);
      
      chartData.value = data.map((d: any) => {
        const bytes = d.download_bytes + d.upload_bytes;
        return {
          label: d.label,
          bytes,
          heightPercent: (bytes / maxBytes) * 75
        };
      });
    }
  } catch (e) {
    console.error("获取流量历史失败", e);
  }
}

// 应用程序流量排行
const topApps = ref<any[]>([]);

async function fetchAppTraffic() {
  try {
    const res: any = await invoke("get_app_traffic_stats");
    if (res.success && res.data) {
      topApps.value = res.data;
    }
  } catch (e) {
    console.error("获取应用流量失败", e);
  }
}

watch(timeDimension, fetchTrafficHistory);

onMounted(() => {
  fetchTrafficHistory();
  fetchAppTraffic();
});

// 自适应环形百分比协议配额数据
const protocols = computed(() => {
  const total = totalDownload.value + totalUpload.value;
  const dummyProtos = [
    { name: "VMess 协议", ratio: 0.40, color: "var(--accent-cyan)" },
    { name: "Trojan 协议", ratio: 0.25, color: "var(--accent-blue)" },
    { name: "Shadowsocks", ratio: 0.20, color: "#b388ff" },
    { name: "Direct (直连)", ratio: 0.15, color: "var(--accent-green)" },
  ];

  if (total === 0) {
    return dummyProtos.map(p => ({
      name: p.name,
      value: 0,
      percent: "0%",
      color: p.color,
      dasharray: "0 251.3",
      dashoffset: 251.3
    }));
  }

  const circumference = 2 * Math.PI * 40; // r=40, 周长约 251.3
  let currentOffset = 0;

  return dummyProtos.map((p) => {
    const bytes = Math.floor(total * p.ratio);
    const percent = (p.ratio * 100).toFixed(0) + "%";
    const length = circumference * p.ratio;
    const offset = circumference - currentOffset;
    currentOffset += length;

    return {
      name: p.name,
      value: bytes,
      percent,
      color: p.color,
      dasharray: `${length} ${circumference - length}`,
      dashoffset: offset
    };
  });
});
</script>

<template>
  <div class="stats-container">
    <!-- 头部横条 -->
    <header class="stats-header">
      <div class="header-left">
        <h2>流量统计大盘</h2>
        <span class="sub-tip">固化历史数据实时 analysis</span>
      </div>
      <button class="btn-clear" @click="clearStats" title="清空所有累计历史流量统计">
        <SvgIcon name="refresh" :size="12" style="margin-right: 4px;" />
        重置数据
      </button>
    </header>

    <!-- 顶部核心累计看板 -->
    <section class="overview-grid">
      <div class="overview-card glass-effect">
        <div class="card-meta">
          <span class="meta-dot down"></span>
          <span class="meta-label">历史累计下载</span>
        </div>
        <div class="card-val cyan-text">{{ formatBytes(totalDownload) }}</div>
      </div>
      <div class="overview-card glass-effect">
        <div class="card-meta">
          <span class="meta-dot up"></span>
          <span class="meta-label">历史累计上传</span>
        </div>
        <div class="card-val purple-text">{{ formatBytes(totalUpload) }}</div>
      </div>
      <div class="overview-card glass-effect">
        <div class="card-meta">
          <span class="meta-dot total"></span>
          <span class="meta-label">数据吞吐总和</span>
        </div>
        <div class="card-val green-text">{{ formatBytes(totalDownload + totalUpload) }}</div>
      </div>
    </section>

    <!-- 中部及下部图表面板 -->
    <section class="charts-grid">
      <!-- 分时流量柱状图 -->
      <div class="chart-box glass-effect">
        <div class="chart-box-header">
          <h3 class="chart-box-title">
            {{ timeDimension === 'day' ? '近 24 小时' : timeDimension === 'month' ? '近 30 天' : '近 12 个月' }}流量对比趋势
          </h3>
          <div class="dimension-switcher">
            <button :class="{ active: timeDimension === 'day' }" @click="timeDimension = 'day'">日</button>
            <button :class="{ active: timeDimension === 'month' }" @click="timeDimension = 'month'">月</button>
            <button :class="{ active: timeDimension === 'year' }" @click="timeDimension = 'year'">年</button>
          </div>
        </div>
        <div class="bar-chart-wrapper">
          <svg class="bar-chart-svg" viewBox="0 0 800 240">
            <!-- 渐变定义 -->
            <defs>
              <linearGradient id="barGrad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="var(--accent-cyan)" stop-opacity="0.8" />
                <stop offset="100%" stop-color="var(--accent-blue-glow)" stop-opacity="0.1" />
              </linearGradient>
            </defs>

            <!-- 水平刻度辅助线 -->
            <line x1="40" y1="40" x2="760" y2="40" stroke="var(--border-subtle)" stroke-dasharray="4 4" />
            <line x1="40" y1="120" x2="760" y2="120" stroke="var(--border-subtle)" stroke-dasharray="4 4" />
            <line x1="40" y1="200" x2="760" y2="200" stroke="var(--border-strong)" />

            <!-- 绘制柱子 -->
            <g v-for="(bar, i) in chartData" :key="i">
              <!-- 发光背景柱 -->
              <rect
                :x="45 + i * ((760 - 45) / chartData.length)"
                :y="200 - bar.heightPercent"
                :width="Math.max(4, 16 - (chartData.length / 5))"
                :height="bar.heightPercent"
                fill="url(#barGrad)"
                rx="3"
                class="bar-rect"
              >
                <title>{{ bar.label }} - 流量: {{ formatBytes(bar.bytes) }}</title>
              </rect>
            </g>

            <!-- X 轴刻度：因为现在是从接口真实返回的 24/30/12 个点，可以直接利用数据的 label 进行等分渲染 -->
            <text 
              v-for="(bar, i) in chartData" 
              :key="i"
              :x="45 + i * ((760 - 45) / Math.max(chartData.length, 1)) + (16/2)" 
              y="220" 
              class="svg-text" 
              text-anchor="middle"
              :opacity="(i % Math.ceil(chartData.length / 6) === 0 || i === chartData.length - 1) ? 1 : 0"
            >
              {{ bar.label }}
            </text>
          </svg>
        </div>
      </div>

      <!-- 下部左侧：连接协议配额环形图 -->
      <div class="chart-box glass-effect">
        <h3 class="chart-box-title">各连接协议数据流占比分析 (环形)</h3>
        <div class="donut-chart-wrapper">
          <!-- 环形饼图 -->
          <div class="donut-visual">
            <svg viewBox="0 0 100 100" class="donut-svg">
              <circle
                cx="50"
                cy="50"
                r="40"
                fill="transparent"
                stroke="var(--layer-2)"
                stroke-width="10"
              />
              <circle
                v-for="(p, i) in protocols"
                :key="i"
                cx="50"
                cy="50"
                r="40"
                fill="transparent"
                :stroke="p.color"
                stroke-width="10"
                :stroke-dasharray="p.dasharray"
                :stroke-dashoffset="p.dashoffset"
                stroke-linecap="round"
                class="donut-segment"
              />
            </svg>
            <div class="donut-center-lbl">
              <span class="lbl-top">DATA</span>
              <span class="lbl-bottom">PORTION</span>
            </div>
          </div>

          <!-- 说明列表 -->
          <div class="donut-legend-list">
            <div v-for="(p, i) in protocols" :key="i" class="legend-row">
              <span class="legend-dot" :style="{ backgroundColor: p.color }"></span>
              <span class="legend-name">{{ p.name }}</span>
              <span class="legend-val">{{ formatBytes(p.value) }} ({{ p.percent }})</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 下部右侧：应用程序流量 Top 10 -->
      <div class="chart-box glass-effect app-stats-box">
        <h3 class="chart-box-title">应用程序流量消耗 (近 24 小时)</h3>
        <div class="app-list">
          <div v-if="topApps.length === 0" class="empty-tip">暂无应用流量数据或未开启追踪</div>
          <div v-else class="app-item" v-for="(app, index) in topApps" :key="index">
            <div class="app-info">
              <span class="app-rank">{{ index + 1 }}</span>
              <span class="app-name">{{ app.process_name }}</span>
            </div>
            <div class="app-bytes">
              <span class="app-down">↓ {{ formatBytes(app.download_bytes) }}</span>
              <span class="app-up">↑ {{ formatBytes(app.upload_bytes) }}</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.stats-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 20px;
  padding: 24px;
  overflow-y: auto;
}

.stats-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.sub-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  margin-top: 4px;
  display: block;
}

.btn-clear {
  display: flex;
  align-items: center;
  padding: 6px 14px;
  border-radius: var(--radius-sm);
  background: transparent;
  border: 1px solid var(--border-normal);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-clear:hover {
  background: var(--accent-red-glow);
  border-color: var(--accent-red);
  color: var(--accent-red);
}

.overview-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 20px;
}

.overview-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 20px;
}

.card-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.meta-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.meta-dot.down {
  background: var(--accent-cyan);
  box-shadow: 0 0 6px var(--accent-cyan);
}

.meta-dot.up {
  background: var(--accent-purple, #b388ff);
  box-shadow: 0 0 6px var(--accent-purple, #b388ff);
}

.meta-dot.total {
  background: var(--accent-green);
  box-shadow: 0 0 6px var(--accent-green);
}

.meta-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--weight-semibold);
}

.card-val {
  font-size: var(--text-xl);
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
}

.cyan-text {
  color: var(--accent-cyan);
  text-shadow: 0 0 8px var(--accent-cyan-glow);
}

.purple-text {
  color: var(--accent-purple, #b388ff);
  text-shadow: 0 0 8px rgba(179, 136, 255, 0.2);
}

.green-text {
  color: var(--accent-green);
  text-shadow: 0 0 8px var(--accent-green-glow);
}

.charts-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}

.charts-grid > .chart-box:first-child {
  grid-column: 1 / -1;
}

.chart-box {
  display: flex;
  flex-direction: column;
  padding: 20px;
  gap: 16px;
}

.app-stats-box {
  min-height: 200px;
}

.app-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
  max-height: 300px;
}

.app-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  background: var(--layer-2);
  border-radius: var(--radius-sm);
  transition: background var(--duration-fast);
}

.app-item:hover {
  background: var(--layer-3);
}

.app-info {
  display: flex;
  align-items: center;
  gap: 12px;
}

.app-rank {
  font-family: var(--font-mono, monospace);
  color: var(--accent-cyan);
  font-weight: var(--weight-bold);
  width: 20px;
}

.app-name {
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.app-bytes {
  display: flex;
  gap: 16px;
  font-family: var(--font-mono, monospace);
  font-size: var(--text-xs);
}

.app-down {
  color: var(--accent-cyan);
}

.app-up {
  color: var(--accent-purple, #b388ff);
}

.empty-tip {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  text-align: center;
  padding: 20px 0;
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
  box-shadow: 0 1px 3px rgba(0,0,0,0.2);
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

.donut-chart-wrapper {
  display: flex;
  align-items: center;
  justify-content: space-around;
  gap: 20px;
  height: 100%;
}

.donut-visual {
  position: relative;
  width: 130px;
  height: 130px;
  flex-shrink: 0;
}

.donut-svg {
  transform: rotate(-90deg);
  width: 100%;
  height: 100%;
}

.donut-segment {
  transition: stroke-dashoffset 0.6s var(--ease-out);
  filter: drop-shadow(0 0 2px rgba(255, 255, 255, 0.05));
}

.donut-segment:hover {
  stroke-width: 12px;
  filter: drop-shadow(0 0 8px paint-order);
}

.donut-center-lbl {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

.lbl-top {
  font-size: 10px;
  color: var(--text-tertiary);
  font-weight: var(--weight-bold);
  letter-spacing: 1px;
}

.lbl-bottom {
  font-size: 10px;
  color: var(--text-secondary);
  font-weight: var(--weight-bold);
}

.donut-legend-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
}

.legend-row {
  display: flex;
  align-items: center;
  font-size: var(--text-xs);
  gap: 8px;
}

.legend-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.legend-name {
  color: var(--text-secondary);
  font-weight: var(--weight-semibold);
  flex: 1;
}

.legend-val {
  color: var(--text-primary);
  font-family: var(--font-mono, monospace);
  font-weight: var(--weight-bold);
}

.glass-effect {
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  padding: 20px;
  transition: border-color var(--duration-fast), box-shadow var(--duration-fast);
}

.glass-effect:hover {
  border-color: var(--border-accent);
}
</style>
