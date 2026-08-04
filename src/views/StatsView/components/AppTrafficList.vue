<script setup lang="ts">
/**
 * 应用程序流量消耗列表
 * 作者: TanXiang
 *
 * 展示应用程序流量 Top 10 排行
 */
import { formatBytes } from "@/utils/format";
import type { AppTrafficItem } from "../hooks/useAppTraffic";

defineProps<{
  topApps: AppTrafficItem[];
}>();
</script>

<template>
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
</style>
