<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
/**
 * 隐私与日志控制面板 — 主入口
 * 作者: TanXiang
 */
import LogConsole from "./components/LogConsole.vue";
import { useLogViewer } from "./hooks/useLogViewer";

const { handleExport } = useLogViewer();
</script>

<template>
  <div class="panel-container">
    <h2><BaseIcon name="ShieldCheck" :size="20" class="panel-header-icon" /> 隐私与日志控制台</h2>

    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>日志自动滚动保留</span>
          <span class="sub-label">系统日志最多在本地保留 7 天</span>
        </div>
        <span class="val">7 天</span>
      </div>

      <div class="setting-item">
        <div class="item-label">
          <span>导出排错诊断日志</span>
          <span class="sub-label">打包内核运行环境与应用日志至桌面以便反馈</span>
        </div>
        <button class="btn-action" @click="handleExport"><BaseIcon name="Download" :size="14" /> 导出诊断包</button>
      </div>
    </div>

    <!-- 实时日志控制台 -->
    <LogConsole />
  </div>
</template>

<style scoped>
.val {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.btn-action {
  padding: var(--space-2) var(--space-3);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-action:hover {
  background: var(--surface-hover);
}
</style>
