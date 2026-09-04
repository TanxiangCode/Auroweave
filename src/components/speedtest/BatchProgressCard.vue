<template>
  <div v-if="visible" class="batch-progress-card glass-effect">
    <div class="progress-info">
      <span>正在测速: <strong>{{ progress.current_node }}</strong></span>
      <span>进度: {{ progress.current_index }} / {{ progress.total }}</span>
    </div>
    <div class="progress-bar-bg">
      <div
        class="progress-bar-fill"
        :style="{ width: `${(progress.current_index / progress.total) * 100}%` }"
      ></div>
    </div>
    <button class="btn-cancel" @click="emit('cancel')">取消测速</button>
  </div>
</template>

<script setup lang="ts">
/**
 * 批量测速进度卡片（共享组件）
 * 作者: TanXiang
 *
 * ProxiesView 与 SpeedtestView 复用（此前两视图各写一份完全相同的模板，
 * 且取消按钮位置/间距有漂移）。数据源统一为 speedtestStore.batchProgress。
 */
import type { BatchProgressPayload } from "@/api/ipc/speedtest";

const props = defineProps<{
  /** 是否显示（isBatchTesting && batchProgress 非空由调用方判断） */
  visible: boolean;
  progress: BatchProgressPayload;
}>();

const emit = defineEmits<{ (e: "cancel"): void }>();

// props 校验：total 为 0 时不渲染（防除零）
if (props.progress && props.progress.total === 0) {
  console.warn("[BatchProgressCard] total=0 不应显示");
}
</script>

<style scoped>
.batch-progress-card {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  margin-bottom: var(--space-3);
}

.progress-info {
  display: flex;
  gap: var(--space-4);
  flex-shrink: 0;
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.progress-info strong {
  color: var(--accent-cyan-vivid);
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.progress-bar-bg {
  flex: 1;
}

.btn-cancel {
  flex-shrink: 0;
  padding: 5px 14px;
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  color: var(--status-danger);
  background: transparent;
  border: 1px solid var(--status-danger);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-cancel:hover {
  background: var(--accent-red-glow);
}
</style>
