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
.panel-container { display: flex; flex-direction: column; gap: var(--space-4); }
h2 { font-size: var(--text-lg); font-weight: var(--weight-bold); }
.setting-group { display: flex; flex-direction: column; gap: var(--space-3); }
/* setting-item / item-label / sub-label 统一走 panel.css 全局定义 */

/* 未实装功能的禁用态开关：走全局胶囊形态，仅叠加禁用视觉 */
.switch { cursor: not-allowed; opacity: 0.5; }

/* "即将支持"标注徽标 */
.coming-soon-tag {
  font-size: 10px;
  font-weight: var(--weight-semibold);
  color: rgba(0, 242, 254, 0.8);
  background: rgba(0, 242, 254, 0.1);
  border: 1px solid rgba(0, 242, 254, 0.25);
  border-radius: var(--radius-xs);
  padding: 0 5px;
  margin-left: 6px;
  vertical-align: 1px;
}
</style>
