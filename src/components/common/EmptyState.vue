<template>
  <div class="empty-state">
    <span v-if="icon" class="state-icon"><BaseIcon :name="icon" :size="40" /></span>
    <p class="state-title">{{ title }}</p>
    <p v-if="description" class="state-desc">{{ description }}</p>
    <button v-if="ctaText" class="state-cta" @click="emit('cta')">
      <BaseIcon v-if="ctaIcon" :name="ctaIcon" :size="14" />
      {{ ctaText }}
    </button>
  </div>
</template>

<script setup lang="ts">
/**
 * 统一空态组件
 * 作者: TanXiang
 *
 * 替代全站四种自造空态结构（豪华卡/SvgIcon/纯文字/一行字）。
 * 图标统一 BaseIcon（禁止 emoji），标题+描述+可选 CTA 的统一节奏。
 */
import BaseIcon from "@/components/common/BaseIcon.vue";

defineProps<{
  /** BaseIcon 图标名（空则不显示图标区） */
  icon?: string;
  title: string;
  description?: string;
  /** CTA 按钮文案（无则不显示按钮） */
  ctaText?: string;
  ctaIcon?: string;
}>();

const emit = defineEmits<{ (e: "cta"): void }>();
</script>

<style scoped>
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-6) var(--space-4);
  text-align: center;
}

.state-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-tertiary);
  opacity: 0.55;
  margin-bottom: var(--space-1);
}

.state-title {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.state-desc {
  margin: 0;
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  max-width: 320px;
  line-height: 1.6;
}

.state-cta {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: var(--space-2);
  padding: 7px 18px;
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  color: var(--accent-cyan-vivid);
  background: var(--accent-cyan-glow);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.state-cta:hover {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 20%, transparent);
  color: var(--text-primary);
}
</style>
