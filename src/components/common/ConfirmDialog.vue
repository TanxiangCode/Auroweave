<template>
  <Teleport to="body">
    <Transition name="confirm-fade">
      <div
        v-if="visible"
        class="modal-backdrop"
      >
        <div class="modal-card confirm-card" role="alertdialog" :aria-label="title">
          <div class="confirm-icon" :class="level">
            <BaseIcon :name="level === 'danger' ? 'AlertTriangle' : 'HelpCircle'" :size="22" />
          </div>
          <h3 class="confirm-title">{{ title }}</h3>
          <p class="confirm-message">{{ message }}</p>
          <div class="confirm-actions">
            <button class="btn text" @click="handleCancel">取消</button>
            <button
              class="btn"
              :class="level === 'danger' ? 'danger-solid' : 'primary'"
              ref="confirmBtnRef"
              @click="handleConfirm"
            >
              {{ confirmText }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
/**
 * 全局确认弹窗（替换原生 confirm() 的统一危险操作确认层）
 * 作者: TanXiang
 *
 * 基于项目 modal 体系（modal-backdrop/modal-card + btn），由 useConfirm() 单例驱动：
 * 全局仅挂载一个实例（App.vue），调用方 await useConfirm().ask({...}) 获得布尔结果。
 * Esc=取消；Enter=确认；danger 级红色主按钮与「不可恢复」文案由调用方保证。
 */
import { ref, onMounted, onUnmounted, nextTick } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";

export interface ConfirmOptions {
  title: string;
  message: string;
  confirmText?: string;
  /** danger=红色确认按钮（危险操作）；normal=蓝色确认（普通询问） */
  level?: "danger" | "normal";
}

const props = defineProps<{
  visible: boolean;
  title: string;
  message: string;
  confirmText: string;
  level: "danger" | "normal";
}>();

const emit = defineEmits<{
  (e: "confirm"): void;
  (e: "cancel"): void;
}>();

const confirmBtnRef = ref<HTMLButtonElement | null>(null);

function handleConfirm() {
  emit("confirm");
}

function handleCancel() {
  emit("cancel");
}

// 键盘语义：Esc=取消，Enter=确认；打开时聚焦确认按钮（Tab 回退可达）
function onKeyDown(e: KeyboardEvent) {
  if (!props.visible) return;
  if (e.key === "Escape") {
    e.preventDefault();
    handleCancel();
  } else if (e.key === "Enter") {
    e.preventDefault();
    handleConfirm();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeyDown);
  await nextTick();
  confirmBtnRef.value?.focus();
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKeyDown);
});
</script>

<style scoped>
.confirm-card {
  width: 400px;
  max-width: 90vw;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: 26px 24px 20px;
  text-align: center;
}

.confirm-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  border-radius: 50%;
}

.confirm-icon.danger {
  color: var(--status-danger);
  background: color-mix(in srgb, var(--status-danger) 10%, transparent);
  border: 1px solid rgba(248, 113, 113, 0.25);
}

.confirm-icon.normal {
  color: var(--accent-cyan-vivid);
  background: var(--accent-cyan-glow, color-mix(in srgb, var(--accent-cyan-vivid) 8%, transparent));
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 25%, transparent);
}

.confirm-title {
  margin: 0;
  font-size: var(--text-base);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.confirm-message {
  margin: 0;
  font-size: var(--text-sm);
  line-height: 1.6;
  color: var(--text-secondary);
}

.confirm-actions {
  display: flex;
  justify-content: center;
  gap: var(--space-3);
  margin-top: var(--space-2);
  width: 100%;
}

.confirm-actions .btn {
  min-width: 96px;
  padding: 8px 18px;
  font-size: var(--text-sm);
  border-radius: var(--radius-sm);
  cursor: pointer;
  border: 1px solid var(--border-normal);
  transition: all var(--duration-fast) var(--ease-out);
}

.btn.text {
  color: var(--text-secondary);
  background: transparent;
}

.btn.text:hover {
  color: var(--text-primary);
  background: var(--layer-3, var(--border-subtle));
}

.btn.primary {
  color: var(--text-primary);
  background: var(--accent-blue);
  border-color: var(--accent-blue);
}

.btn.primary:hover {
  filter: brightness(1.15);
}

/* 危险确认：红色实底（与危险语义唯一对应的确认按钮形态） */
.btn.danger-solid {
  color: var(--text-primary);
  background: var(--accent-red);
  border-color: var(--accent-red);
}

.btn.danger-solid:hover {
  background: #f05252;
  filter: brightness(1.08);
}

/* 进出场动效 */
.confirm-fade-enter-active,
.confirm-fade-leave-active {
  transition: opacity var(--duration-normal) var(--ease-out);
}

.confirm-fade-enter-active .modal-card,
.confirm-fade-leave-active .modal-card {
  transition: transform var(--duration-normal) var(--ease-out);
}

.confirm-fade-enter-from,
.confirm-fade-leave-to {
  opacity: 0;
}

.confirm-fade-enter-from .modal-card {
  transform: scale(0.96);
}
</style>
