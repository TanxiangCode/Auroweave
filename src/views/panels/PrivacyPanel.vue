<template>
  <div class="panel-container">
    <h2>🛡️ 隐私与日志控制台</h2>
    
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
        <button class="btn-action" @click="handleExport">📥 导出诊断包</button>
      </div>
    </div>

    <!-- 实时日志控制台区 -->
    <div class="log-console-container glass-effect">
      <div class="console-header">
        <div class="header-left">
          <span class="console-title">📁 实时运行日志</span>
          <div class="selector-group">
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
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useToast } from "@/composables/useToast";
import { info as logInfo, error as logError } from "@tauri-apps/plugin-log";

const toast = useToast();

type LogTab = "app" | "service";

const activeTab = ref<LogTab>("app");
const logContent = ref<string>("");
const logViewer = ref<HTMLElement | null>(null);

const tabs = [
  { key: "app" as LogTab, label: "应用与前端日志 (auroweave.log)" },
  { key: "service" as LogTab, label: "系统服务日志 (service.log)" },
];

async function handleExport() {
  logInfo("[PrivacyPanel] 用户尝试导出诊断包...");
  toast.info("正在生成诊断日志包...");
  try {
    const res: any = await invoke("settings_export_diagnostic_log");
    if (res.success) {
      logInfo(`[PrivacyPanel] 导出诊断包成功，路径: ${res.data}`);
      toast.success("导出成功", `已保存至: ${res.data || '桌面'}`);
    } else {
      logError(`[PrivacyPanel] 导出诊断包失败: ${res.error}`);
      toast.error("导出失败", res.error);
    }
  } catch (e: any) {
    logError(`[PrivacyPanel] 导出诊断包接口调用错误: ${e}`);
    toast.error("导出失败", e.message || String(e));
  }
}

// 抓取日志内容
async function fetchLogs() {
  const methodMap = {
    app: "log_read_app",
    service: "log_read_service",
  };
  const currentMethod = methodMap[activeTab.value];
  try {
    const res: any = await invoke(currentMethod, { lines: 250 });
    if (res.success) {
      logContent.value = res.data || "";
      // 自动滚动到日志底部
      nextTick(() => {
        if (logViewer.value) {
          logViewer.value.scrollTop = logViewer.value.scrollHeight;
        }
      });
    } else {
      logContent.value = `加载日志失败: ${res.error}`;
    }
  } catch (e: any) {
    logContent.value = `加载日志异常: ${e.message || e}`;
  }
}

// 一键清理所有日志
async function clearAllLogs() {
  const confirmClear = confirm("您确定要清空主程序日志、系统服务日志以及内核运行日志吗？(此操作不可逆)");
  if (!confirmClear) return;
  
  toast.info("正在清空所有日志...");
  try {
    const res: any = await invoke("log_clear_all");
    if (res.success) {
      toast.success("清空日志成功");
      logContent.value = "";
      logInfo("[PrivacyPanel] 成功清空了所有本地日志文件");
    } else {
      toast.error("清空日志失败", res.error);
    }
  } catch (e: any) {
    toast.error("清空日志失败", e.message || String(e));
  }
}

// 监听标签页切换
watch(activeTab, () => {
  fetchLogs();
});

onMounted(() => {
  fetchLogs();
});
</script>

<style scoped>
.panel-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

h2 {
  font-size: 18px;
  font-weight: 700;
}

.setting-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
}

.item-label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 14px;
  font-weight: 600;
}

.sub-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  font-weight: normal;
}

.val {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.7);
}

.btn-action {
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: #fff;
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s ease;
}
.btn-action:hover {
  background: rgba(255, 255, 255, 0.15);
}

/* 实时日志控制台 */
.log-console-container {
  margin-top: 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  background: rgba(10, 10, 15, 0.4);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.console-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background: rgba(255, 255, 255, 0.02);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 16px;
}

.console-title {
  font-size: 13px;
  font-weight: 700;
  color: rgba(255, 255, 255, 0.9);
}

.selector-group {
  display: flex;
  background: rgba(255, 255, 255, 0.04);
  padding: 2px;
  border-radius: 8px;
  gap: 2px;
}

.tab-btn {
  padding: 4px 10px;
  background: transparent;
  border: none;
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.6);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.tab-btn.active {
  background: rgba(255, 255, 255, 0.08);
  color: #00f2fe;
  font-weight: 600;
}

.header-right {
  display: flex;
  gap: 8px;
}

.btn-tool {
  padding: 4px 10px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: rgba(255, 255, 255, 0.85);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s ease;
}
.btn-tool:hover {
  background: rgba(255, 255, 255, 0.12);
}
.btn-tool.danger {
  color: #ff5c5c;
  border-color: rgba(255, 92, 92, 0.2);
}
.btn-tool.danger:hover {
  background: rgba(255, 92, 92, 0.12);
}

.console-body {
  height: 280px;
  padding: 14px;
  overflow-y: auto;
  font-family: Consolas, Monaco, "Courier New", Courier, monospace;
  font-size: 12px;
  line-height: 1.5;
  background: rgba(0, 0, 0, 0.2);
  color: #d1d5db;
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
  color: rgba(255, 255, 255, 0.35);
  font-style: italic;
}
</style>
