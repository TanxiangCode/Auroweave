<template>
  <div class="panel-container">
    <h2>⌨️ 全局快捷键</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>Spotlight 命令框热键 <span class="fixed-tag">当前固定</span></span>
          <span class="sub-label">快速呼出极速搜索面板（快捷键暂不支持自定义）</span>
        </div>
        <kbd class="hotkey-kbd">Ctrl + Space</kbd>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
</script>

<style scoped>
.hotkey-kbd {
  padding: var(--space-1) var(--space-3);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--accent-cyan-vivid);
}

/* "当前固定"标注徽标 */
.fixed-tag {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-tertiary);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  padding: 0 5px;
  margin-left: 6px;
  vertical-align: 1px;
}
</style>
