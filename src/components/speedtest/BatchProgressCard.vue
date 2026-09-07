<template>
  <div v-if="visible" class="batch-progress-card glass-effect">
    <div class="progress-info">
      <span>{{ actionLabel }}: <strong>{{ progress.current_node }}</strong></span>
      <span>进度: {{ progress.current_index }} / {{ progress.total }}</span>
    </div>
    <div class="progress-bar-bg">
      <div
        class="progress-bar-fill"
        :style="{ width: `${(progress.current_index / progress.total) * 100}%` }"
      ></div>
    </div>
    <button class="btn-cancel" @click="emit('cancel')">{{ cancelLabel }}</button>
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
  }>(),
  {
    actionLabel: "正在测速",
    cancelLabel: "取消测速",
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
