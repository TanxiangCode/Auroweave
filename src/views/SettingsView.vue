<script setup lang="ts">
/**
 * 设置视图（九大面板）
 * 作者: TanXiang
 * TODO(模块H): 实现胶囊变形转场 + 九大设置面板
 */
</script>
<template>
  <div class="view-placeholder">
    <div class="placeholder-icon">⚙️</div>
    <h1>设置</h1>
    <p>模块 H 开发中……</p>
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
