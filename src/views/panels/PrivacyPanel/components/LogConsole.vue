<script setup lang="ts">
/**
 * 实时日志控制台
 * 作者: TanXiang
 *
 * 包含：Tab 切换（应用/服务日志）、刷新、清空、日志内容展示
 */
import { useLogViewer } from "../hooks/useLogViewer";

const {
  activeTab,
  logContent,
  logViewer,
  tabs,
  fetchLogs,
  clearAllLogs,
} = useLogViewer();
</script>

<template>
  <div class="log-console-container glass-effect">
    <div class="console-header">
      <div class="header-left">
        <span class="console-title">📁 实时运行日志</span>
        <div class="tab-group compact">
          <button
            v-for="tab in tabs"
            :key="tab.key"
            class="tab-btn"
            :class="{ active: activeTab === tab.key }"
            @click="activeTab = tab.key"
          >
            {{ tab.label }}
          </button>
        </div>
      </div>
      <div class="header-right">
        <button class="btn-tool" @click="fetchLogs">🔄 刷新</button>
        <button class="btn-tool danger" @click="clearAllLogs">🗑️ 一键清空所有日志</button>
      </div>
    </div>

    <!-- 日志显示框 -->
    <div class="console-body" ref="logViewer">
      <pre v-if="logContent.trim()">{{ logContent }}</pre>
      <div v-else class="empty-log">暂无日志或文件尚不存在</div>
    </div>
  </div>
</template>

<style scoped>
.log-console-container {
  margin-top: var(--space-3);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  background: var(--surface-inset);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.console-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-3) var(--space-4);
  background: var(--surface-inset);
  border-bottom: 1px solid var(--border-subtle);
}

.header-left {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.console-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.tab-group.compact {
  padding: 2px;
  gap: 2px;
  border-radius: var(--radius-sm);
}

.tab-group.compact .tab-btn {
  padding: 4px 10px;
  font-size: var(--text-xs);
  border-radius: var(--radius-xs);
}

.header-right {
  display: flex;
  gap: var(--space-2);
}

.btn-tool {
  padding: 4px 10px;
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-tool:hover {
  background: var(--border-strong);
}

.btn-tool.danger {
  color: var(--status-danger);
  border-color: var(--accent-red-glow);
}

.btn-tool.danger:hover {
  background: var(--accent-red-glow);
}

.console-body {
  height: 280px;
  padding: var(--space-4);
  overflow-y: auto;
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: var(--leading-normal);
  background: var(--surface-inset);
  color: var(--text-secondary);
}

.console-body pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-all;
}

.empty-log {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-tertiary);
  font-style: italic;
}
</style>
