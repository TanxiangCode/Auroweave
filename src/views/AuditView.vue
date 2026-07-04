<script setup lang="ts">
/**
 * 安全审计视图（语义化安全看板）
 * 作者: TanXiang
 * TODO(模块G): 接入 SemanticRuleCard / RawLogStream / connection_counter
 */
</script>
<template>
  <div class="view-placeholder">
    <div class="placeholder-icon">🔍</div>
    <h1>安全审计</h1>
    <p>模块 G 开发中……</p>
  </div>
</template>
<style scoped>
.view-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: var(--space-4);
  color: var(--text-secondary);
}
.placeholder-icon { font-size: 48px; }
h1 { font-size: var(--text-xl); color: var(--text-primary); }
p { font-size: var(--text-sm); }
</style>
