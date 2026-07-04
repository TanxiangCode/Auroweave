<template>
  <div class="panel-container">
    <h2>📡 DNS 安全与解析</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>远端加密 DNS (Remote DoH)</span>
          <span class="sub-label">海外加密解析，防 DNS 污染污染 (默认: https://1.1.1.1/dns-query)</span>
        </div>
        <input type="text" value="https://1.1.1.1/dns-query" class="text-input" readonly />
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>本地直连 DNS (Local UDP)</span>
          <span class="sub-label">国内零延迟解析 (默认: udp://223.5.5.5)</span>
        </div>
        <input type="text" value="223.5.5.5" class="text-input" readonly />
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
.text-input { padding: 6px 12px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: rgba(255,255,255,0.8); font-size: 12px; width: 220px; outline: none; }
</style>
