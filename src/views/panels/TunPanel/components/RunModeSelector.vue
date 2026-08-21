<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
/**
 * 内核运行模式选择器
 * 作者: TanXiang
 */
defineProps<{
  runMode: string;
  operating: boolean;
}>();

const emit = defineEmits<{
  select: [mode: "local" | "service"];
}>();
</script>

<template>
  <div class="mode-selector-wrap">
    <button
      class="mode-btn"
      :class="{ active: runMode === 'local' }"
      :disabled="operating"
      @click="emit('select', 'local')"
    >
      <span class="btn-title"><BaseIcon name="Cpu" :size="15" /> 本地运行模式 (Local)</span>
      <span class="btn-desc">GUI 结合提权任务托管内核子进程。开启 TUN 需要首次提权配置计划任务，此后即免弹窗运行。</span>
    </button>
    <button
      class="mode-btn"
      :class="{ active: runMode === 'service' }"
      :disabled="operating"
      @click="emit('select', 'service')"
    >
      <span class="btn-title"><BaseIcon name="Shield" :size="15" /> 系统服务模式 (Service)</span>
      <span class="btn-desc">由独立的 Windows 系统服务托管内核，日常开启/关闭 TUN 均免 UAC 二次弹窗，支持随开机自启运行。</span>
    </button>
  </div>
</template>

<style scoped>
.mode-selector-wrap {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
  margin-top: var(--space-1);
}

.mode-btn {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  background: var(--surface-inset);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  cursor: pointer;
  text-align: left;
  transition: all 0.25s var(--ease-default);
  outline: none;
}

.mode-btn:hover {
  background: var(--surface-hover);
  border-color: var(--border-strong);
  transform: translateY(-2px);
}

.mode-btn.active {
  background: var(--accent-cyan-glow);
  border-color: var(--accent-cyan-vivid);
  color: var(--text-primary);
  box-shadow: 0 4px 20px var(--accent-cyan-glow);
}

.btn-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  display: flex;
  align-items: center;
}

.btn-desc {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  line-height: var(--leading-normal);
}

.mode-btn.active .btn-desc {
  color: var(--text-secondary);
}
</style>
