<template>
  <div class="audit-compact-banner glass-effect">
    <!-- 左侧：多维指标水平陈列 -->
    <div class="metrics-row">
      <!-- 活跃连接 -->
      <div class="metric-item">
        <span class="badge-dot pulse"></span>
        <span class="metric-label">活跃</span>
        <span class="metric-val active-val">{{ activeCount }}</span>
      </div>

      <div class="divider"></div>

      <!-- 节点加密 -->
      <div class="metric-item" title="经节点加密转发的海外连接">
        <span class="metric-icon">🚀</span>
        <span class="metric-label">加密代理</span>
        <span class="metric-val text-cyan">{{ proxiedCount }}</span>
      </div>

      <div class="divider"></div>

      <!-- 大陆直连 -->
      <div class="metric-item" title="大陆直连无中转连接">
        <span class="metric-icon">🎯</span>
        <span class="metric-label">大陆直连</span>
        <span class="metric-val text-green">{{ directCount }}</span>
      </div>

      <div class="divider"></div>

      <!-- 安全阻断 -->
      <div class="metric-item" title="广告与恶意域名拦截">
        <span class="metric-icon">🚫</span>
        <span class="metric-label">安全阻断</span>
        <span class="metric-val text-red">{{ blockedCount }}</span>
      </div>

      <div class="divider"></div>

      <!-- 实时总吞吐 -->
      <div class="metric-item throughput" title="瞬时网络总吞吐速率">
        <span class="metric-icon">⚡</span>
        <div class="speed-group">
          <span class="speed-down">↓ {{ downloadSpeed }}</span>
          <span class="speed-up">↑ {{ uploadSpeed }}</span>
        </div>
      </div>
    </div>

    <!-- 右侧：流控与操作按钮组 -->
    <div class="actions-group">
      <button
        class="btn-compact pause"
        :class="{ active: isPaused }"
        @click="$emit('toggle-pause')"
        :title="isPaused ? '点击恢复实时连接流' : '点击暂停实时连接流'"
      >
        <span class="btn-icon">{{ isPaused ? '▶️' : '⏸️' }}</span>
        <span>{{ isPaused ? '已暂停流' : '暂停流' }}</span>
      </button>

      <button
        class="btn-compact danger"
        :disabled="activeCount === 0"
        @click="$emit('close-all')"
        title="切断当前所有活跃网络连接"
      >
        <span class="btn-icon">🔌</span>
        <span>全部切断</span>
      </button>

      <button
        class="btn-compact secondary"
        @click="$emit('clear-history')"
        title="清空已记录的历史审计条目"
      >
        <span class="btn-icon">🗑️</span>
        <span>清空</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  activeCount: number;
  proxiedCount: number;
  directCount: number;
  blockedCount: number;
  downloadSpeed: string;
  uploadSpeed: string;
  isPaused: boolean;
}>();

defineEmits<{
  (e: "toggle-pause"): void;
  (e: "close-all"): void;
  (e: "clear-history"): void;
}>();
</script>

<style scoped>
.audit-compact-banner {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 14px;
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 12px;
  gap: 12px;
  flex-shrink: 0;
}

.metrics-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.metric-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
}

.metric-icon {
  font-size: 13px;
}

.metric-label {
  color: rgba(255, 255, 255, 0.55);
  font-size: 11px;
}

.metric-val {
  font-weight: 700;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  color: rgba(255, 255, 255, 0.95);
}

.active-val {
  color: #10b981;
}

.text-cyan { color: #00f2fe !important; }
.text-green { color: #10b981 !important; }
.text-red { color: #f87171 !important; }

.divider {
  width: 1px;
  height: 14px;
  background: rgba(255, 255, 255, 0.1);
}

.throughput .speed-group {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

.speed-down { color: #00f2fe; }
.speed-up { color: #a78bfa; }

.badge-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #10b981;
}

.badge-dot.pulse {
  animation: pulse-glow 2s infinite;
}

@keyframes pulse-glow {
  0% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7); }
  70% { transform: scale(1.2); box-shadow: 0 0 0 5px rgba(16, 185, 129, 0); }
  100% { transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }
}

.actions-group {
  display: flex;
  align-items: center;
  gap: 6px;
}

.btn-compact {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 9px;
  font-size: 11px;
  font-weight: 500;
  border-radius: 7px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.8);
  cursor: pointer;
  transition: all 0.15s ease;
}

.btn-compact:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.09);
  border-color: rgba(255, 255, 255, 0.18);
  color: #fff;
}

.btn-compact:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}

.btn-compact.pause.active {
  background: rgba(245, 158, 11, 0.15);
  border-color: #f59e0b;
  color: #f59e0b;
}

.btn-compact.danger:hover:not(:disabled) {
  background: rgba(239, 68, 68, 0.18);
  border-color: #ef4444;
  color: #ef4444;
}
</style>
