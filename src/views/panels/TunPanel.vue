<template>
  <div class="panel-container">
    <h2>🔌 TUN 虚拟网卡</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>TUN 网卡接口名</span>
          <span class="sub-label">默认为 auroweave-tun</span>
        </div>
        <input type="text" value="auroweave-tun" class="text-input" readonly />
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>网卡 MTU</span>
          <span class="sub-label">最大传输单元，默认 9000</span>
        </div>
        <input type="text" value="9000" class="text-input" readonly />
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
.text-input { padding: 6px 12px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: rgba(255,255,255,0.8); font-size: 12px; width: 140px; outline: none; }
</style>
