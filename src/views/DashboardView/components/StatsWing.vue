<script setup lang="ts">
/**
 * 右翼统计区
 * 作者: TanXiang
 *
 * 展示活动连接数和累计流量，点击可跳转
 */
import { formatBytes } from "@/utils/format";

defineProps<{
  activeConnectionCount: number;
  totalDownload: number;
  totalUpload: number;
}>();

const emit = defineEmits<{
  navigate: [path: string];
}>();
</script>

<template>
  <div class="stats-wing">
    <!-- 胶囊 1: 活动连接 -->
    <div
      class="stat-pill clickable-pill"
      @click="emit('navigate', '/audit')"
      title="点击查看实时连接审计"
    >
      <span class="pill-label">活动连接</span>
      <span class="pill-val cyan-glow">{{ activeConnectionCount }} 条</span>
    </div>

    <!-- 胶囊 2: 累计流量 -->
    <div
      class="stat-pill clickable-pill"
      @click="emit('navigate', '/stats')"
      title="点击查看详细流量统计"
    >
      <span class="pill-label">累计流量</span>
      <span class="pill-val data-combined-val">
        <span class="down-flow">↓ {{ formatBytes(totalDownload) }}</span>
        <span class="divider">|</span>
        <span class="up-flow">↑ {{ formatBytes(totalUpload) }}</span>
      </span>
    </div>
  </div>
</template>

<style scoped>
.stats-wing {
  display: flex;
  flex-direction: column;
  gap: 24px;
  justify-content: flex-end;
  height: 100%;
  padding-bottom: 10px;
}

.stat-pill {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  height: 48px;
  transition: all var(--duration-fast) var(--ease-out);
}

.stat-pill.clickable-pill {
  cursor: pointer;
}

.stat-pill.clickable-pill:hover {
  background: var(--layer-2);
  border-color: var(--accent-blue);
  box-shadow: 0 0 12px var(--accent-blue-glow);
  transform: translateX(-4px);
}

.pill-label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-weight: var(--weight-bold);
  letter-spacing: 0.5px;
}

.pill-val {
  font-size: var(--text-xs);
  color: var(--text-primary);
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
}

.pill-val.cyan-glow {
  color: var(--accent-cyan);
  text-shadow: 0 0 6px var(--accent-cyan-glow);
}

.data-combined-val {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: var(--weight-bold);
  font-family: var(--font-mono, monospace);
}

.down-flow {
  color: var(--accent-cyan);
  text-shadow: 0 0 4px var(--accent-cyan-glow);
}

.up-flow {
  color: var(--accent-purple, #b388ff);
  text-shadow: 0 0 4px rgba(179, 136, 255, 0.25);
}

.divider {
  color: var(--border-strong);
  font-weight: var(--weight-light);
  opacity: 0.5;
}
</style>
