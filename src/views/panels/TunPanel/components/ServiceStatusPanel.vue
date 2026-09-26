<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
/**
 * 系统服务状态面板
 * 作者: TanXiang
 *
 * 展示服务运行状态网格与控制按钮
 */
defineProps<{
  serviceStatus: {
    installedVersion: string | null;
    lastKnownStatus: string;
    lastFallbackReason: string | null;
  };
  statusText: string;
  operating: boolean;
}>();

const emit = defineEmits<{
  start: [];
  stop: [];
  'view-log': [];
  uninstall: [];
  install: [];
}>();
</script>

<template>
  <div class="service-manager-box">
    <h3><BaseIcon name="Shield" :size="16" /> 系统服务状态控制</h3>

    <div class="status-grid">
      <!-- 状态显示 -->
      <div class="grid-item status-indicator">
        <span class="grid-label">当前状态</span>
        <div class="status-value-wrap">
          <span class="pulse-dot" :class="serviceStatus.lastKnownStatus"></span>
          <span class="status-label-text">{{ statusText }}</span>
        </div>
      </div>

      <!-- 版本显示 -->
      <div class="grid-item">
        <span class="grid-label">已安装版本</span>
        <span class="grid-value">{{ serviceStatus.installedVersion || "未检测到" }}</span>
      </div>

      <!-- 故障指示 -->
      <div v-if="serviceStatus.lastFallbackReason" class="grid-item full-width fallback-alert">
        最近一次回退原因:
        <strong>
          {{ serviceStatus.lastFallbackReason === 'not_installed' ? '未安装系统服务' : '服务启动失败' }}
        </strong>
      </div>
    </div>

    <!-- 状态操作控制按钮 -->
    <div class="action-buttons">
      <button
        v-if="serviceStatus.lastKnownStatus === 'stopped'"
        class="control-btn success"
        :disabled="operating"
        @click="emit('start')"
      >
        <BaseIcon name="Play" :size="12" /> 启动服务
      </button>
      <button
        v-if="serviceStatus.lastKnownStatus === 'running'"
        class="control-btn danger"
        :disabled="operating"
        @click="emit('stop')"
      >
        <BaseIcon name="Square" :size="12" /> 停止服务
      </button>
      <button
        class="control-btn secondary"
        :disabled="operating"
        @click="emit('view-log')"
      >
        查看服务日志
      </button>
      <button
        v-if="serviceStatus.lastKnownStatus !== 'not_installed'"
        class="control-btn outline-danger"
        :disabled="operating"
        @click="emit('uninstall')"
      >
        卸载服务
      </button>
      <button
        v-else
        class="control-btn success"
        :disabled="operating"
        @click="emit('install')"
      >
        安装服务
      </button>
    </div>
  </div>
</template>

<style scoped>
.service-manager-box {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-4);
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  margin-top: var(--space-1);
}

.service-manager-box h3 {
  font-size: var(--text-base);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
  border-left: 3px solid var(--accent-cyan-vivid);
  padding-left: var(--space-2);
  margin: 0;
}

.status-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
}

.grid-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  background: var(--surface-inset);
  border: 1px solid var(--border-subtle);
  padding: var(--space-3) var(--space-3);
  border-radius: var(--radius-sm);
}

.grid-item.full-width {
  grid-column: span 2;
}

.grid-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.grid-value {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.status-value-wrap {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.status-label-text {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
}

/* 呼吸点 */
.pulse-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-tertiary);
}

.pulse-dot.running {
  background: var(--accent-green);
  box-shadow: 0 0 8px var(--accent-green-glow);
  animation: pulse 2s infinite;
}

.pulse-dot.stopped {
  background: var(--text-tertiary);
}

.pulse-dot.not_installed {
  background: var(--accent-orange);
  box-shadow: 0 0 8px var(--accent-orange);
}

.pulse-dot.error {
  background: var(--accent-red);
  box-shadow: 0 0 8px var(--accent-red-glow);
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0% { transform: scale(0.95); box-shadow: 0 0 0 0 var(--accent-green-glow); }
  70% { transform: scale(1); box-shadow: 0 0 0 6px transparent; }
  100% { transform: scale(0.95); box-shadow: 0 0 0 0 transparent; }
}

.fallback-alert {
  background: var(--accent-cyan-glow) !important;
  border-color: var(--border-accent) !important;
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

/* 按钮操作区 */
.action-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.control-btn {
  padding: var(--space-2) var(--space-4);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
  outline: none;
  border: 1px solid transparent;
}

.control-btn:hover:not(:disabled) {
  transform: translateY(-1px);
}

.control-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.control-btn.success {
  background: var(--accent-green);
  color: var(--text-on-accent);
}
.control-btn.success:hover:not(:disabled) {
  opacity: 0.85;
}

.control-btn.danger {
  background: var(--accent-red);
  color: var(--text-on-accent);
}
.control-btn.danger:hover:not(:disabled) {
  opacity: 0.85;
}

.control-btn.secondary {
  background: var(--surface-hover);
  border-color: var(--border-strong);
  color: var(--text-primary);
}
.control-btn.secondary:hover:not(:disabled) {
  background: var(--surface-hover);
}

.control-btn.outline-danger {
  background: transparent;
  border-color: var(--accent-red-glow);
  color: var(--status-danger);
}
.control-btn.outline-danger:hover:not(:disabled) {
  background: var(--accent-red-glow);
}
</style>
