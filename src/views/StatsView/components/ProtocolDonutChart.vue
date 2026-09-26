<script setup lang="ts">
/**
 * 连接协议配额环形图
 * 作者: TanXiang
 *
 * 纯 SVG 绘制的动态环形图 + 图例列表
 */
import { computed } from "vue";
import { formatBytes } from "@/utils/format";

const props = defineProps<{
  totalDownload: number;
  totalUpload: number;
}>();

/**
 * 协议配额配置（占比例）
 * 注意：当前为固定示意比例，后端尚未提供按协议维度的流量统计数据。
 * 若未来后端增加协议分布统计（如各 outbound 协议的流量字节数），
 * 将此数组替换为真实数据聚合即可。
 */
const dummyProtos = [
  { name: "VMess 协议", ratio: 0.40, color: "var(--accent-cyan)" },
  { name: "Trojan 协议", ratio: 0.25, color: "var(--accent-blue)" },
  { name: "Shadowsocks", ratio: 0.20, color: "#b388ff" },
  { name: "Direct (直连)", ratio: 0.15, color: "var(--accent-green)" },
];

/** 自适应环形百分比协议配额数据 */
const protocols = computed(() => {
  const total = props.totalDownload + props.totalUpload;
  const circumference = 2 * Math.PI * 40; // r=40, 周长约 251.3

  if (total === 0) {
    return dummyProtos.map((p) => ({
      name: p.name,
      value: 0,
      percent: "0%",
      color: p.color,
      dasharray: "0 251.3",
      dashoffset: 251.3,
    }));
  }

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
      dashoffset: offset,
    };
  });
});
</script>

<template>
  <div class="chart-box glass-effect">
    <h3 class="chart-box-title">
      各连接协议数据流占比分析 (环形)
      <span class="demo-badge" title="当前后端未提供按协议维度的流量统计，图为示意比例">示意数据</span>
    </h3>
    <div class="donut-chart-wrapper">
      <div class="donut-visual">
        <svg viewBox="0 0 100 100" class="donut-svg">
          <circle cx="50" cy="50" r="40" fill="transparent" stroke="var(--layer-2)" stroke-width="10" />
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

      <div class="donut-legend-list">
        <div v-for="(p, i) in protocols" :key="i" class="legend-row">
          <span class="legend-dot" :style="{ backgroundColor: p.color }"></span>
          <span class="legend-name">{{ p.name }}</span>
          <span class="legend-val">{{ formatBytes(p.value) }} ({{ p.percent }})</span>
        </div>
      </div>
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

.chart-box-title {
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-weight: var(--weight-bold);
  display: flex;
  align-items: center;
  gap: 6px;
}

/* 示意数据徽标：诚实标注当前图为固定示意比例，非真实统计 */
.demo-badge {
  font-size: 9px;
  font-weight: var(--weight-semibold);
  color: var(--text-tertiary);
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  padding: 1px 5px;
  letter-spacing: 0.5px;
  flex-shrink: 0;
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
  filter: drop-shadow(0 0 2px var(--border-subtle));
}

.donut-segment:hover {
  stroke-width: 12px;
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
</style>
