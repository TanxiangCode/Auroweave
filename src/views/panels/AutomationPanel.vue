<template>
  <div class="panel-container">
    <h2><BaseIcon name="Zap" :size="20" class="panel-header-icon" /> 场景自动化引擎</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>家庭 Wi-Fi 自动化规避 <span class="coming-soon-tag">即将支持</span></span>
          <span class="sub-label">连接到 HomeNetwork 时自动切换为直连模式（功能开发中，暂不可配置）</span>
        </div>
        <input type="checkbox" class="switch" disabled />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
</script>

<style scoped>
.panel-container { display: flex; flex-direction: column; gap: 16px; }
h2 { font-size: 18px; font-weight: 700; }
.setting-group { display: flex; flex-direction: column; gap: 12px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-weight: 600; }
.sub-label { font-size: 11px; color: rgba(255,255,255,0.4); font-weight: normal; }
.switch { width: 18px; height: 18px; cursor: not-allowed; opacity: 0.5; }

/* "即将支持"标注徽标 */
.coming-soon-tag {
  font-size: 10px;
  font-weight: 600;
  color: rgba(0, 242, 254, 0.8);
  background: rgba(0, 242, 254, 0.1);
  border: 1px solid rgba(0, 242, 254, 0.25);
  border-radius: 4px;
  padding: 0 5px;
  margin-left: 6px;
  vertical-align: 1px;
}
</style>
