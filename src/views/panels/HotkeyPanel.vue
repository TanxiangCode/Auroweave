<template>
  <div class="panel-container">
    <h2>⌨️ 全局快捷键</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>Spotlight 命令框热键</span>
          <span class="sub-label">快速呼出极速搜索面板</span>
        </div>
        <kbd class="hotkey-kbd">Ctrl + Space</kbd>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
</script>

<style scoped>
.panel-container { display: flex; flex-direction: column; gap: 16px; }
h2 { font-size: 18px; font-weight: 700; }
.setting-group { display: flex; flex-direction: column; gap: 12px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-weight: 600; }
.sub-label { font-size: 11px; color: rgba(255,255,255,0.4); font-weight: normal; }
.hotkey-kbd { padding: 4px 10px; background: rgba(255,255,255,0.1); border-radius: 6px; font-family: monospace; font-size: 13px; color: #00f2fe; }
</style>
