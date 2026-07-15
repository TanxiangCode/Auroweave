<script setup lang="ts">
/**
 * TUN 网卡配置与系统服务管理面板
 * 作者: TanXiang
 */

import { ref, onMounted, onUnmounted, computed } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import {
  serviceQueryStatus,
  serviceInstall,
  serviceUninstall,
  serviceStart,
  serviceStop,
  serviceReadLog
} from "@/api/ipc/settings";

const settingsStore = useSettingsStore();

// 读写本地网卡名称
const localName = ref(settingsStore.settings.tun_interface_name || "Auroweave");
const saving = ref(false);
const saved = ref(false);
let saveTimer: ReturnType<typeof setTimeout> | null = null;

// 系统服务相关的状态
const serviceStatus = ref({
  installedVersion: null as string | null,
  lastKnownStatus: "not_installed" as string,
  lastFallbackReason: null as string | null,
});

const showConfirmModal = ref(false);
const showLogModal = ref(false);
const serviceLog = ref("");
const operating = ref(false);
const errorMsg = ref("");

let statusTimer: ReturnType<typeof setInterval> | null = null;

const runMode = computed(() => {
  return settingsStore.settings.core?.runMode || "local";
});

onMounted(() => {
  localName.value = settingsStore.settings.tun_interface_name || "Auroweave";
  refreshStatus();
  statusTimer = setInterval(refreshStatus, 3000);
});

onUnmounted(() => {
  if (statusTimer) clearInterval(statusTimer);
});

async function refreshStatus() {
  try {
    const res = await serviceQueryStatus();
    if (res.success && res.data) {
      serviceStatus.value = res.data;
    }
  } catch (e) {
    console.error("轮询服务状态异常:", e);
  }
}

// 切换模式控制流
async function selectMode(mode: "local" | "service") {
  if (operating.value) return;
  errorMsg.value = "";
  operating.value = true;

  setTimeout(async () => {
    try {
      if (mode === "local") {
        const patch = {
          core: {
            runMode: "local" as const,
            service: { ...settingsStore.settings.core.service, lastKnownStatus: "stopped" as const }
          }
        };
        // 乐观更新
        settingsStore.settings.core.runMode = "local";
        await settingsStore.updateSettings(patch);
        await refreshStatus();
      } else {
        const res = await serviceQueryStatus();
        if (res.success && res.data) {
          serviceStatus.value = res.data;
          if (res.data.lastKnownStatus === "not_installed") {
            // 未安装：弹出 UAC 提权询问
            showConfirmModal.value = true;
          } else {
            // 已安装：直接切换并存盘
            const patch = {
              core: {
                ...settingsStore.settings.core,
                runMode: "service" as const
              }
            };
            // 乐观更新
            settingsStore.settings.core.runMode = "service";
            await settingsStore.updateSettings(patch);
          }
        }
      }
    } catch (e: any) {
      errorMsg.value = e.message || "切换运行模式失败";
      // 发生错误时重新拉取设置，恢复正确的 UI 状态
      await settingsStore.fetchSettings();
    } finally {
      operating.value = false;
    }
  }, 50);
}

// UAC 安装服务
async function handleInstallService() {
  showConfirmModal.value = false;
  operating.value = true;
  errorMsg.value = "";
  try {
    const res = await serviceInstall();
    if (res.success) {
      // 后端在 service_install 中已经修改了 settings，这里刷新 store 并重载状态即可
      await settingsStore.fetchSettings();
      // 强制更新并保存当前运行模式为系统服务模式
      const patch = {
        core: {
          ...settingsStore.settings.core,
          runMode: "service" as const
        }
      };
      await settingsStore.updateSettings(patch);
      await refreshStatus();
    } else {
      errorMsg.value = res.error || "服务安装未成功，UAC 授权可能已被取消";
    }
  } catch (e: any) {
    errorMsg.value = e.message || "执行安装服务请求时发生错误";
  } finally {
    operating.value = false;
  }
}

function handleCancelInstall() {
  showConfirmModal.value = false;
}

// 启停控制动作
async function handleStart() {
  operating.value = true;
  errorMsg.value = "";
  try {
    const res = await serviceStart();
    if (res.success) {
      await refreshStatus();
    } else {
      errorMsg.value = res.error || "启动服务失败";
    }
  } catch (e: any) {
    errorMsg.value = e.message || "启动服务异常";
  } finally {
    operating.value = false;
  }
}

async function handleStop() {
  operating.value = true;
  errorMsg.value = "";
  try {
    const res = await serviceStop();
    if (res.success) {
      await refreshStatus();
    } else {
      errorMsg.value = res.error || "停止服务失败";
    }
  } catch (e: any) {
    errorMsg.value = e.message || "停止服务异常";
  } finally {
    operating.value = false;
  }
}

async function handleUninstall() {
  if (!confirm("确定要卸载 Auroweave 系统服务吗？此操作需要管理员权限。")) return;
  operating.value = true;
  errorMsg.value = "";
  try {
    const res = await serviceUninstall();
    if (res.success) {
      await settingsStore.fetchSettings();
      await refreshStatus();
    } else {
      errorMsg.value = res.error || "卸载服务失败";
    }
  } catch (e: any) {
    errorMsg.value = e.message || "卸载服务异常";
  } finally {
    operating.value = false;
  }
}

async function handleViewLog() {
  operating.value = true;
  try {
    const res = await serviceReadLog();
    if (res.success && res.data) {
      serviceLog.value = res.data;
      showLogModal.value = true;
    } else {
      errorMsg.value = res.error || "无法加载服务日志";
    }
  } catch (e: any) {
    errorMsg.value = e.message || "加载服务日志异常";
  } finally {
    operating.value = false;
  }
}

// 防抖保存网卡名称
function onNameInput() {
  saved.value = false;
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(saveName, 800);
}

async function saveName() {
  const trimmed = localName.value.trim();
  if (!trimmed) {
    localName.value = "Auroweave";
  }
  saving.value = true;
  await settingsStore.updateSettings({ tun_interface_name: localName.value });
  saving.value = false;
  saved.value = true;
  setTimeout(() => { saved.value = false; }, 2000);
}

// 格式化状态显示
const statusText = computed(() => {
  switch (serviceStatus.value.lastKnownStatus) {
    case "running": return "运行中";
    case "stopped": return "已停止";
    case "not_installed": return "未安装";
    case "error": return "异常";
    default: return "检测中";
  }
});
</script>

<template>
  <div class="panel-container">
    <h2>🔌 内核运行模式</h2>
    <p class="panel-desc">
      选择 GUI 主程序与 sing-box 内核的交互架构。在 Windows 下，推荐使用系统服务模式以实现免 UAC 的 TUN 网络接管体验。
    </p>

    <!-- 运行模式切换控制区 -->
    <div class="mode-selector-wrap">
      <button 
        class="mode-btn" 
        :class="{ active: runMode === 'local' }"
        :disabled="operating"
        @click="selectMode('local')"
      >
        <span class="btn-title">💻 本地运行模式 (Local)</span>
        <span class="btn-desc">GUI 结合提权任务托管内核子进程。开启 TUN 需要首次提权配置计划任务，此后即免弹窗运行。</span>
      </button>
      <button 
        class="mode-btn" 
        :class="{ active: runMode === 'service' }"
        :disabled="operating"
        @click="selectMode('service')"
      >
        <span class="btn-title">🛡️ 系统服务模式 (Service)</span>
        <span class="btn-desc">由独立的 Windows 系统服务托管内核，日常开启/关闭 TUN 均免 UAC 二次弹窗，支持随开机自启运行。</span>
      </button>
    </div>

    <!-- 提示直连模式开启 TUN 的小警告 -->
    <div v-if="runMode === 'local' && settingsStore.settings.tun_enabled" class="fallback-banner warning">
      ⚠️ 当前为本地运行模式，如果您尚未一键提权安装组件，启用 TUN 将提示提权。推荐切换至<strong>系统服务模式</strong>以获得流畅的开机自启体验。
    </div>

    <!-- 切换失败错误展示 -->
    <div v-if="errorMsg" class="fallback-banner error">
      ❌ 操作失败: {{ errorMsg }}
    </div>

    <!-- 系统服务详细状态面板 -->
    <Transition name="fade-slide">
      <div v-if="runMode === 'service'" class="service-manager-box">
        <h3>🛡️ 系统服务状态控制</h3>
        
        <div class="status-grid">
          <!-- 状态显示 -->
          <div class="grid-item status-indicator">
            <span class="grid-label">当前状态</span>
            <div class="status-value-wrap">
              <span class="pulse-dot" :class="serviceStatus.lastKnownStatus"></span>
              <span class="status-label-text">{{ statusText }}</span>
            </div>
          </div>

          <!-- 版本显示 -->
          <div class="grid-item">
            <span class="grid-label">已安装版本</span>
            <span class="grid-value">{{ serviceStatus.installedVersion || "未检测到" }}</span>
          </div>

          <!-- 故障指示 -->
          <div v-if="serviceStatus.lastFallbackReason" class="grid-item full-width fallback-alert">
            💡 最近一次回退原因: 
            <strong>
              {{ serviceStatus.lastFallbackReason === 'not_installed' ? '未安装系统服务' : '服务启动失败' }}
            </strong>
          </div>
        </div>

        <!-- 状态操作控制按钮 -->
        <div class="action-buttons">
          <button 
            v-if="serviceStatus.lastKnownStatus === 'stopped'"
            class="control-btn success"
            :disabled="operating"
            @click="handleStart"
          >
            ▶ 启动服务
          </button>
          <button 
            v-if="serviceStatus.lastKnownStatus === 'running'"
            class="control-btn danger"
            :disabled="operating"
            @click="handleStop"
          >
            ⏹ 停止服务
          </button>
          <button 
            class="control-btn secondary"
            :disabled="operating"
            @click="handleViewLog"
          >
            📋 查看服务日志
          </button>
          <button 
            v-if="serviceStatus.lastKnownStatus !== 'not_installed'"
            class="control-btn outline-danger"
            :disabled="operating"
            @click="handleUninstall"
          >
            🗑️ 卸载服务
          </button>
          <button 
            v-else
            class="control-btn success"
            :disabled="operating"
            @click="selectMode('service')"
          >
            🛠️ 安装服务
          </button>
        </div>
      </div>
    </Transition>

    <hr class="separator" />

    <h2>🔌 TUN 虚拟网卡</h2>
    <p class="panel-desc">
      TUN 模式通过虚拟网卡在系统内核层接管流量，配置对应的参数选项：
    </p>

    <div class="setting-group">
      <!-- 网卡接口名 -->
      <div class="setting-item">
        <div class="item-label">
          <span>虚拟网卡名称</span>
          <span class="sub-label">显示在 Windows 网络适配器列表中，修改后重启 TUN 生效</span>
        </div>
        <div class="input-wrap">
          <input
            id="tun-interface-name"
            v-model="localName"
            type="text"
            class="text-input"
            placeholder="Auroweave"
            maxlength="32"
            @input="onNameInput"
            @blur="saveName"
          />
          <span v-if="saving" class="status-text saving">保存中…</span>
          <span v-else-if="saved" class="status-text saved">✓ 已保存</span>
        </div>
      </div>
    </div>

    <!-- 为什么只需授权一次？科普卡片 -->
    <div class="info-card">
      <div class="info-icon">💡</div>
      <div class="info-body">
        <p><strong>关于免 UAC 系统服务工作机制：</strong></p>
        <p>
          Auroweave 采用 Windows 自定义安全描述符（DACL）机制。安装服务时会授予当前会话用户对该服务的启动与停止权限。
          配置为系统服务模式后，前端操作日常代理的开启/关闭，只需直接驱动后台服务启停而完全不需要系统弹出 UAC 管理员提权确认，既安全又优雅。
        </p>
      </div>
    </div>

    <!-- UAC 提权安装确认 Modal -->
    <div v-if="showConfirmModal" class="glass-modal-overlay">
      <div class="glass-modal">
        <div class="modal-header">
          <span class="modal-icon">🛡️</span>
          <h3>安装 Auroweave 系统服务</h3>
        </div>
        <div class="modal-body">
          <p>启用系统服务模式需要安装 <strong>Auroweave Core Service</strong> 到您的 Windows 系统中。</p>
          <p class="warning-text">⚠️ 此操作需要申请一次系统管理员权限（弹出 UAC 窗口）。完成安装后，日常使用中启用/关闭 TUN 均不再弹出该提示。</p>
          <p>是否立即开始安装？</p>
        </div>
        <div class="modal-actions">
          <button class="modal-btn confirm" @click="handleInstallService">是，立即安装</button>
          <button class="modal-btn cancel" @click="handleCancelInstall">否，取消</button>
        </div>
      </div>
    </div>

    <!-- 服务运行日志 Modal -->
    <div v-if="showLogModal" class="glass-modal-overlay large">
      <div class="glass-modal log-modal">
        <div class="modal-header">
          <span class="modal-icon">📋</span>
          <h3>系统服务运行日志</h3>
          <button class="close-x" @click="showLogModal = false">×</button>
        </div>
        <div class="modal-body log-body">
          <pre class="log-content">{{ serviceLog || "暂无日志内容" }}</pre>
        </div>
        <div class="modal-actions">
          <button class="modal-btn secondary" @click="handleViewLog">🔄 刷新</button>
          <button class="modal-btn cancel" @click="showLogModal = false">关闭</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.panel-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

h2, h3 {
  font-weight: 700;
  margin: 0;
}
h2 {
  font-size: 18px;
}
h3 {
  font-size: 15px;
  color: rgba(255, 255, 255, 0.85);
  border-left: 3px solid #00f2fe;
  padding-left: 8px;
}

.panel-desc {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
  line-height: 1.6;
  margin: 0;
}

.separator {
  border: 0;
  height: 1px;
  background: rgba(255, 255, 255, 0.08);
  margin: 12px 0;
}

/* 模式选择按钮 */
.mode-selector-wrap {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-top: 4px;
}

.mode-btn {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 16px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  text-align: left;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  outline: none;
}

.mode-btn:hover {
  background: rgba(255, 255, 255, 0.05);
  border-color: rgba(255, 255, 255, 0.15);
  transform: translateY(-2px);
}

.mode-btn.active {
  background: rgba(0, 242, 254, 0.08);
  border-color: rgba(0, 242, 254, 0.4);
  color: #fff;
  box-shadow: 0 4px 20px rgba(0, 242, 254, 0.1);
}

.btn-title {
  font-size: 14px;
  font-weight: 600;
  display: flex;
  align-items: center;
}

.btn-desc {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
  line-height: 1.5;
}

.mode-btn.active .btn-desc {
  color: rgba(255, 255, 255, 0.6);
}

/* 错误与回退横幅 */
.fallback-banner {
  padding: 10px 14px;
  font-size: 12px;
  border-radius: 8px;
  line-height: 1.5;
}

.fallback-banner.warning {
  background: rgba(245, 158, 11, 0.08);
  border: 1px solid rgba(245, 158, 11, 0.25);
  color: #f59e0b;
}

.fallback-banner.error {
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.25);
  color: #ef4444;
}

/* 系统服务管理卡片 */
.service-manager-box {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 16px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  margin-top: 4px;
}

.status-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.grid-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.05);
  padding: 10px 12px;
  border-radius: 8px;
}

.grid-item.full-width {
  grid-column: span 2;
}

.grid-label {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.45);
}

.grid-value {
  font-size: 13px;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
}

.status-value-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}

.status-label-text {
  font-size: 13px;
  font-weight: 600;
}

/* 呼吸点 */
.pulse-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #6b7280;
}

.pulse-dot.running {
  background: #10b981;
  box-shadow: 0 0 8px rgba(16, 185, 129, 0.6);
  animation: pulse 2s infinite;
}

.pulse-dot.stopped {
  background: #6b7280;
}

.pulse-dot.not_installed {
  background: #f59e0b;
  box-shadow: 0 0 8px rgba(245, 158, 11, 0.4);
}

.pulse-dot.error {
  background: #ef4444;
  box-shadow: 0 0 8px rgba(239, 68, 68, 0.6);
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0% {
    transform: scale(0.95);
    box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7);
  }
  70% {
    transform: scale(1);
    box-shadow: 0 0 0 6px rgba(16, 185, 129, 0);
  }
  100% {
    transform: scale(0.95);
    box-shadow: 0 0 0 0 rgba(16, 185, 129, 0);
  }
}

.fallback-alert {
  background: rgba(0, 242, 254, 0.03) !important;
  border-color: rgba(0, 242, 254, 0.1) !important;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.65);
}

/* 按钮操作区 */
.action-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.control-btn {
  padding: 8px 14px;
  font-size: 12px;
  font-weight: 600;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
  outline: none;
  border: 1px solid transparent;
}

.control-btn:hover:not(:disabled) {
  transform: translateY(-1px);
}

.control-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.control-btn.success {
  background: #10b981;
  color: #fff;
}
.control-btn.success:hover:not(:disabled) {
  background: #059669;
}

.control-btn.danger {
  background: #ef4444;
  color: #fff;
}
.control-btn.danger:hover:not(:disabled) {
  background: #dc2626;
}

.control-btn.secondary {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.12);
  color: rgba(255, 255, 255, 0.85);
}
.control-btn.secondary:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
}

.control-btn.outline-danger {
  background: transparent;
  border-color: rgba(239, 68, 68, 0.3);
  color: #f87171;
}
.control-btn.outline-danger:hover:not(:disabled) {
  background: rgba(239, 68, 68, 0.1);
}

/* 传统网卡配置 */
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
  max-width: 280px;
  line-height: 1.4;
}

.input-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.text-input {
  padding: 7px 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.9);
  font-size: 13px;
  width: 150px;
  outline: none;
  transition: border-color 0.15s;
}

.text-input:focus {
  border-color: rgba(0, 242, 254, 0.4);
}

.status-text {
  font-size: 11px;
  white-space: nowrap;
}

.saving {
  color: rgba(255, 255, 255, 0.4);
}

.saved {
  color: #4ade80;
}

/* 提示卡片 */
.info-card {
  display: flex;
  gap: 12px;
  padding: 14px 16px;
  background: rgba(0, 242, 254, 0.05);
  border: 1px solid rgba(0, 242, 254, 0.15);
  border-radius: 12px;
}

.info-icon {
  font-size: 18px;
  flex-shrink: 0;
  padding-top: 1px;
}

.info-body {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.info-body p {
  margin: 0;
  font-size: 12px;
  color: rgba(255, 255, 255, 0.6);
  line-height: 1.6;
}

.info-body strong {
  color: rgba(0, 242, 254, 0.9);
}

/* 毛玻璃弹窗 */
.glass-modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(20px);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 1000;
}

.glass-modal {
  width: 420px;
  background: rgba(25, 28, 36, 0.85);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 16px;
  padding: 24px;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.glass-modal-overlay.large .glass-modal {
  width: 680px;
  max-height: 80vh;
}

.modal-header {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 16px;
  position: relative;
}

.modal-icon {
  font-size: 22px;
}

.modal-header h3 {
  font-size: 16px;
  font-weight: 700;
}

.close-x {
  position: absolute;
  right: 0;
  background: transparent;
  border: 0;
  color: rgba(255, 255, 255, 0.4);
  font-size: 22px;
  cursor: pointer;
}

.close-x:hover {
  color: #fff;
}

.modal-body {
  font-size: 13px;
  color: rgba(255, 255, 255, 0.7);
  line-height: 1.6;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.warning-text {
  color: #f59e0b;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.modal-btn {
  padding: 8px 16px;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  outline: none;
  border: 1px solid transparent;
}

.modal-btn.confirm {
  background: #00f2fe;
  color: #0d0f12;
}
.modal-btn.confirm:hover {
  background: #05d5e0;
}

.modal-btn.cancel {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.12);
  color: rgba(255, 255, 255, 0.85);
}
.modal-btn.cancel:hover {
  background: rgba(255, 255, 255, 0.12);
}

.modal-btn.secondary {
  background: transparent;
  border-color: rgba(0, 242, 254, 0.3);
  color: #00f2fe;
}
.modal-btn.secondary:hover {
  background: rgba(0, 242, 254, 0.08);
}

/* 日志查看器 */
.log-body {
  overflow: hidden;
}

.log-content {
  margin: 0;
  padding: 12px;
  background: rgba(0, 0, 0, 0.3);
  border: 1px solid rgba(255, 255, 255, 0.05);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.8);
  font-family: Consolas, Monaco, monospace;
  font-size: 11px;
  line-height: 1.5;
  height: 380px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

/* 动效 */
.fade-slide-enter-active,
.fade-slide-leave-active {
  transition: all 0.3s ease;
}

.fade-slide-enter-from,
.fade-slide-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
