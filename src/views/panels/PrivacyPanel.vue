<template>
  <div class="panel-container">
    <h2>🛡️ 隐私与日志导出</h2>
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
  </div>
</template>

<script setup lang="ts">
import { exportDiagnosticLog } from "@/api/ipc/settings";
import { useToast } from "@/composables/useToast";

const toast = useToast();

async function handleExport() {
  toast.info("正在生成诊断日志包...");
  const res = await exportDiagnosticLog();
  if (res.success) {
    toast.success("导出成功", `已保存至: ${res.data || '系统日志目录'}`);
  } else {
    toast.error("导出失败", res.error);
  }
}
</script>

<style scoped>
.panel-container { display: flex; flex-direction: column; gap: 16px; }
h2 { font-size: 18px; font-weight: 700; }
.setting-group { display: flex; flex-direction: column; gap: 12px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-weight: 600; }
.sub-label { font-size: 11px; color: rgba(255,255,255,0.4); font-weight: normal; }
.val { font-size: 13px; color: rgba(255,255,255,0.7); }
.btn-action { padding: 6px 12px; background: rgba(255,255,255,0.08); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #fff; font-size: 12px; cursor: pointer; }
</style>
