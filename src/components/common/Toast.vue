<template>
  <div class="toast-container">
    <TransitionGroup name="toast">
      <div
        v-for="toast in toasts"
        :key="toast.id"
        class="toast-item"
        :class="toast.type"
        @click="remove(toast.id)"
      >
        <div class="toast-icon">
          <span v-if="toast.type === 'success'"><BaseIcon name="Check" :size="16" /></span>
          <span v-else-if="toast.type === 'error'"><BaseIcon name="X" :size="14" /></span>
          <span v-else-if="toast.type === 'warning'"><BaseIcon name="AlertTriangle" :size="16" /></span>
          <span v-else>ℹ</span>
        </div>
        <div class="toast-content">
          <div class="toast-title">{{ toast.title }}</div>
          <div v-if="toast.message" class="toast-message">{{ toast.message }}</div>
        </div>
      </div>
    </TransitionGroup>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useToast } from "@/composables/useToast";

const { toasts, remove } = useToast();
</script>

<style scoped>
.toast-container {
  position: fixed;
  bottom: 24px;
  right: 24px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 10px;
  pointer-events: none;
}

.toast-item {
  pointer-events: auto;
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 18px;
  min-width: 280px;
  max-width: 400px;
  border-radius: var(--radius-lg, 12px);
  background: rgba(18, 22, 34, 0.85);
  backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4);
  cursor: pointer;
  transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

.toast-item.success {
  border-color: rgba(74, 222, 128, 0.3);
}
.toast-item.success .toast-icon {
  color: #4ade80;
}

.toast-item.error {
  border-color: rgba(248, 113, 113, 0.3);
}
.toast-item.error .toast-icon {
  color: #f87171;
}

.toast-item.warning {
  border-color: rgba(251, 191, 36, 0.3);
}
.toast-item.warning .toast-icon {
  color: #fbbf24;
}

.toast-item.info {
  border-color: rgba(56, 189, 248, 0.3);
}
.toast-item.info .toast-icon {
  color: #38bdf8;
}

.toast-icon {
  font-size: 16px;
  font-weight: bold;
  line-height: 1;
  margin-top: 2px;
}

.toast-content {
  flex: 1;
}

.toast-title {
  font-size: 14px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.92);
}

.toast-message {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.6);
  margin-top: 2px;
  line-height: 1.4;
  word-break: break-word;
}

/* 动画过渡 */
.toast-enter-from {
  opacity: 0;
  transform: translateY(20px) scale(0.95);
}
.toast-enter-active {
  transition: all 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}
.toast-leave-active {
  transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(30px);
}
</style>
