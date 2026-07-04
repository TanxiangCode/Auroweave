<script setup lang="ts">
/**
 * 代理节点视图
 * 作者: TanXiang
 * TODO(模块C/E): 接入节点列表、测速按钮
 */
</script>
<template>
  <div class="view-placeholder">
    <div class="placeholder-icon">🌐</div>
    <h1>代理节点</h1>
    <p>模块 C / E 开发中……</p>
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
