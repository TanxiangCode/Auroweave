<template>
  <div v-if="visible" class="batch-progress-card glass-effect">
    <div class="progress-info">
      <span class="action-label">{{ actionLabel }}</span>
      <strong :title="cancelling ? '取消中，等待当前节点结束' : progress.current_node">
        {{ cancelling ? "取消中，等待当前节点结束…" : progress.current_node }}
      </strong>
      <span class="progress-count">{{ progress.current_index }} / {{ progress.total }}</span>
    </div>
    <div class="progress-bar-bg">
      <div
        class="progress-bar-fill"
        :style="{ width: `${(progress.current_index / progress.total) * 100}%` }"
      ></div>
    </div>
    <button class="btn-cancel" :disabled="cancelling" @click="emit('cancel')">
      {{ cancelling ? "取消中…" : cancelLabel }}
    </button>
  </div>
</template>

<script setup lang="ts">
/**
 * 批量任务进度卡片（共享组件）
 * 作者: TanXiang
 *
 * ProxiesView / SpeedtestView 复用（测速与解锁检测两个批量任务共用形态）。
 * 进度结构同构：current_index/total/current_node 必有，result 类型随任务而异，
 * 本组件只消费三个进度字段，故以最小结构类型收窄（结构化兼容两种 payload）。
 */
import type { ThroughputResult } from "@/types";

/** 组件实际消费的最小进度结构（测速/解锁检测 payload 均结构化兼容） */
export interface BatchProgressLike {
  current_index: number;
  total: number;
  current_node: string;
  result?: ThroughputResult | unknown;
}

const props = withDefaults(
  defineProps<{
    /** 是否显示（批量进行中且 batchProgress 非空由调用方判断） */
    visible: boolean;
    progress: BatchProgressLike;
    /** 动作名（"正在测速"/"解锁检测"） */
    actionLabel?: string;
    /** 取消按钮文案 */
    cancelLabel?: string;
    /** 已发出取消请求、等待后端终止事件：按钮锁定并显示"取消中" */
    cancelling?: boolean;
  }>(),
  {
    actionLabel: "正在测速",
    cancelLabel: "取消测速",
    cancelling: false,
  }
);

const emit = defineEmits<{ (e: "cancel"): void }>();

// props 校验：total 为 0 时不渲染（防除零）
if (props.progress && props.progress.total === 0) {
  console.warn("[BatchProgressCard] total=0 不应显示");
}
</script>

<style scoped>
.batch-progress-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(32px, 72px) auto;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: var(--radius-md);
  min-width: 0;
  overflow: hidden;
  box-sizing: border-box;
}

.progress-info {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  overflow: hidden;
  font-size: var(--text-xs);
  color: var(--text-secondary);
  white-space: nowrap;
}

.action-label,
.progress-count {
  flex-shrink: 0;
}

.action-label {
  color: var(--text-primary);
}

.progress-info strong {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--accent-cyan-vivid);
  font-weight: var(--weight-medium);
}

.progress-bar-bg {
  width: 100%;
  min-width: 40px;
  height: 6px;
  background: var(--bg-surface-elevated, rgba(255, 255, 255, 0.08));
  border-radius: var(--radius-full, 9999px);
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--accent-cyan-vivid), var(--accent-blue-vivid, #3b82f6));
  border-radius: var(--radius-full, 9999px);
  transition: width 0.2s ease-out;
}

.btn-cancel {
  min-width: 32px;
  padding: 3px 6px;
  font-size: 11px;
  font-weight: var(--weight-semibold);
  color: var(--status-danger);
  background: transparent;
  border: 1px solid var(--status-danger);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-cancel:hover:not(:disabled) {
  background: var(--accent-red-glow);
}

.btn-cancel:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
