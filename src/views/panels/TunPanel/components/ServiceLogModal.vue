<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
/**
 * 服务运行日志弹窗
 * 作者: TanXiang
 */
defineProps<{
  visible: boolean;
  logContent: string;
}>();

const emit = defineEmits<{
  close: [];
  refresh: [];
}>();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop">
      <div class="modal-card glass-effect log-modal">
        <div class="modal-header">
          <span class="modal-icon"></span>
          <h3>系统服务运行日志</h3>
          <button class="close-x" @click="emit('close')">×</button>
        </div>
        <div class="modal-body log-body">
          <pre class="log-content">{{ logContent || "暂无日志内容" }}</pre>
        </div>
        <div class="modal-actions">
          <button class="btn text" @click="emit('refresh')"><BaseIcon name="RefreshCw" :size="14" /> 刷新</button>
          <button class="btn text" @click="emit('close')">关闭</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.log-modal {
  width: 680px;
  max-height: 80vh;
}

.modal-icon {
  font-size: var(--text-xl);
}

.close-x {
  position: absolute;
  right: 0;
  background: transparent;
  border: 0;
  color: var(--text-tertiary);
  font-size: var(--text-xl);
  cursor: pointer;
}

.close-x:hover {
  color: var(--text-primary);
}

.log-body {
  overflow: hidden;
}

.log-content {
  margin: 0;
  padding: var(--space-3);
  background: var(--surface-inset);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: var(--leading-normal);
  height: 380px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
