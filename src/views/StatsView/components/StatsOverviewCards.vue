<script setup lang="ts">
/**
 * 累计流量看板
 * 作者: TanXiang
 *
 * 顶部三宫格：下载 / 上传 / 总和
 */
import { formatBytes } from "@/utils/format";

defineProps<{
  totalDownload: number;
  totalUpload: number;
}>();
</script>

<template>
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
</template>

<style scoped>
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
</style>
