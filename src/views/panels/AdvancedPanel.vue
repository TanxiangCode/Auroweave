<template>
  <div class="panel-container">
    <!-- 1. Sing-box 内核版本与在线升级管理 -->
    <div class="setting-card glass-effect highlight-border">
      <div class="card-header">
        <span class="card-icon"><BaseIcon name="FlaskConical" :size="20" /></span>
        <div class="card-title-group">
          <h3>Sing-box 内核版本管理</h3>
          <p>检测官方 GitHub Release 最新内核版本并提供一键在线升级与热重启</p>
        </div>
        <button class="btn-check-update" @click="handleCheckUpdate" :disabled="checkingUpdate || upgrading">
          <span :class="{ spinning: checkingUpdate }"></span>
          <span>{{ checkingUpdate ? '正在检查...' : '检查更新' }}</span>
        </button>
      </div>

      <div class="card-body">
        <div class="kernel-status-row">
          <div class="status-left">
            <span class="status-label">当前运行内核：</span>
            <span class="version-badge current">v{{ currentVersion }}</span>
            <span class="status-text">100% 规则对齐 (Official Sidecar)</span>
          </div>

          <div class="status-right" v-if="updateInfo">
            <span v-if="updateInfo.has_update" class="update-found-badge">
              发现新版本 {{ updateInfo.latest_version }}
            </span>
            <span v-else class="up-to-date-badge"><BaseIcon name="Check" :size="13" /> 已是最新版本</span>
          </div>
        </div>

        <!-- JSON Schema 导出（plan-Q Q1）：产出 Draft 2020-12 文件供外部编辑器校验 -->
        <div class="setting-item schema-export-row">
          <div class="item-label">
            <span>导出 JSON Schema</span>
            <span class="sub-label">生成本内核的配置校验文件（config/schema.json），VS Code 等外部编辑器加载后可对 config.json 字段级补全校验</span>
          </div>
          <button class="btn-restore" @click="handleExportSchema" :disabled="exportingSchema">
            <span>{{ exportingSchema ? '正在导出...' : '导出 Schema' }}</span>
          </button>
        </div>

        <!-- 升级通知卡片（发现新版本时显示） -->
        <div v-if="updateInfo && updateInfo.has_update" class="update-release-box">
          <div class="release-header">
            <div class="release-title">
              <span class="release-tag">{{ updateInfo.latest_version }}</span>
              <span class="release-date">发布于 {{ formatDate(updateInfo.published_at) }}</span>
            </div>
            <button
              class="btn-upgrade-now"
              @click="handleUpgrade"
              :disabled="upgrading"
            >
              <span v-if="!upgrading">立即在线升级</span>
              <span v-else class="upgrading-state">
                <span class="spinner"></span>
                正在下载并替换内核 (请勿关闭)...
              </span>
            </button>
          </div>

          <!-- 更新日志摘要 -->
          <div class="release-notes" v-if="updateInfo.release_notes">
            <div class="notes-title">更新日志 (Changelog):</div>
            <pre class="notes-content">{{ updateInfo.release_notes }}</pre>
          </div>
        </div>
      </div>
    </div>

    <!-- 延迟测试与吞吐量测速 / AI 服务解锁检测判据 已抽离至 SpeedtestPanel.vue（测速与解锁） -->

    <!-- 3. 系统性能与灾备恢复 -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"><BaseIcon name="FlaskConical" :size="20" /></span>
        <div class="card-title-group">
          <h3>性能模式与灾备恢复</h3>
          <p>控制 GPU 毛玻璃渲染与异常配置一键还原</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item" :class="{ highlight: highlightTopology }">
          <div class="item-label">
            <span>启用拓扑网络图 (Topology Canvas)</span>
            <span class="sub-label">在分流页面启用全景 4 层动态交互式贝塞尔流光拓扑画布</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.topology_enabled"
            class="switch"
            @change="save"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>性能低耗模式 (Performance Mode)</span>
            <span class="sub-label">一键关闭背景毛玻璃与高耗 GPU/CPU 动画滤镜，适应低功耗场景</span>
          </div>
          <input
            type="checkbox"
            v-model="settingsStore.settings.performance_mode"
            class="switch"
            @change="handlePerfModeChange"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>恢复上一次配置备份</span>
            <span class="sub-label">若当前内核配置文件异常，一键恢复 config.backup.json 并重启</span>
          </div>
          <button class="btn-restore" @click="handleRestore" :disabled="restoring">
            <span>{{ restoring ? '正在恢复...' : '恢复备份' }}</span>
          </button>
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>编辑当前内核配置</span>
            <span class="sub-label">手改 config.json（保存前经内核 check 校验，拦截拒载配置），改动前内容自动入备份</span>
          </div>
          <button class="btn-restore" @click="showConfigEditor = true">
            <span>打开编辑器</span>
          </button>
        </div>
      </div>
    </div>

    <!-- 测速探测内核端口（高级/排障项，后端 scheduler.rs 与 unlock_check.rs 均消费） -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon"><BaseIcon name="Cpu" :size="20" /></span>
        <div class="card-title-group">
          <h3>测速探测内核端口</h3>
          <p>独立 test-core 实例的监听端口基址，仅在默认端口被占用或与本机其他服务冲突时才需修改</p>
        </div>
      </div>

      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>test-core 端口基址</span>
            <span class="sub-label">
              批量延迟/解锁探测会为每个探测实例分配 {{ testCorePortBase || 40040 }} 起的连续端口；
              设为 0 表示使用内置默认值。修改后需重启测速任务生效。
            </span>
          </div>
          <input
            v-model.number="settingsStore.settings.test_core_port_base"
            type="number"
            class="num-input"
            min="0"
            max="65000"
            step="1"
            @change="handlePortBaseChange"
          />
        </div>

        <div class="marker-hint">
          <strong>排障提示：</strong>若测速批量任务报「端口被占用」或启动失败，可把基址改成其他空闲高位端口
          （如 41000）。批量探测会在基址之后按并发数顺延分配，单节点测速另用基址 +500 的端口段，二者不冲突。
        </div>
      </div>
    </div>

    <!-- 配置编辑器弹窗（plan-Q Q2） -->
    <ConfigEditorModal :visible="showConfigEditor" @close="showConfigEditor = false" />
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { computed, ref, watch, onMounted } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";
import { useConfirm } from "@/composables/useConfirm";
import { invoke } from "@tauri-apps/api/core";
import {
  checkSingboxUpdate,
  restoreConfigBackup,
} from "@/api/ipc/settings";
import { exportConfigSchema } from "@/api/ipc/configEditor";
import { invokeWithTimeout } from "@/api/ipc/client";
import type { SingboxUpdateInfo } from "@/types";
import ConfigEditorModal from "./ConfigEditorModal.vue";

const props = defineProps<{
  highlightTarget?: string;
}>();

const settingsStore = useSettingsStore();
const toast = useToast();

const currentVersion = ref<string>("加载中...");
const highlightTopology = ref(false);

/** test-core 端口基址：0 / 未设置时回落到后端内置默认 40040，仅用于文案展示 */
const testCorePortBase = computed(() => settingsStore.settings.test_core_port_base || 0);

/**
 * 校验并保存 test-core 端口基址
 *
 * 该端口段仅用于测速探测实例，误配会导致批量测速启动失败；
 * 这里做区间收敛并把非法值回退为 0（=后端默认），避免用户填入越界值后无从恢复。
 */
async function handlePortBaseChange() {
  const raw = settingsStore.settings.test_core_port_base ?? 0;
  const value = Number.isFinite(raw) ? Math.trunc(raw) : 0;
  if (value < 0 || value > 65000) {
    settingsStore.settings.test_core_port_base = 0;
    toast.error("端口基址超出范围", "已重置为默认值 0（后端按 40040 起算）。");
    return;
  }
  settingsStore.settings.test_core_port_base = value;
  const res = await settingsStore.updateSettings({ test_core_port_base: value });
  if (res.success) {
    toast.success(
      "端口基址已保存",
      value === 0 ? "已恢复默认（后端按 40040 起算）" : `批量探测将从 ${value} 起分配端口`
    );
  }
}
const checkingUpdate = ref(false);
const upgrading = ref(false);
const restoring = ref(false);
const exportingSchema = ref(false);
const showConfigEditor = ref(false);
const updateInfo = ref<SingboxUpdateInfo | null>(null);

function formatDate(dateStr: string): string {
  if (!dateStr) return "";
  try {
    const d = new Date(dateStr);
    return d.toLocaleDateString("zh-CN", { year: "numeric", month: "long", day: "numeric" });
  } catch {
    return dateStr;
  }
}

async function fetchCurrentVersion() {
  try {
    const res = await invoke<any>("proxy_get_singbox_version");
    if (res.success && res.data) {
      currentVersion.value = res.data;
    } else {
      currentVersion.value = "未知";
    }
  } catch {
    currentVersion.value = "无法获取";
  }
}

async function handleCheckUpdate() {
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  toast.info("正在查询 GitHub Release 最新版本...");
  try {
    const res = await checkSingboxUpdate();
    if (res.success && res.data) {
      updateInfo.value = res.data;
      if (res.data.has_update) {
        toast.info(`发现新版本 ${res.data.latest_version}`, "点击「立即升级」可一键自动更新内核");
      } else {
        toast.success("当前已是最新内核版本", `v${res.data.current_version}`);
      }
    } else {
      toast.error("检查更新失败", res.error || "无法连接到 GitHub API");
    }
  } catch (e) {
    toast.error("检查更新失败", e instanceof Error ? e.message : String(e));
  } finally {
    checkingUpdate.value = false;
  }
}

async function handleUpgrade() {
  if (!updateInfo.value || !updateInfo.value.download_url) {
    toast.error("未找到对应平台的下载资产");
    return;
  }

  upgrading.value = true;
  toast.info("正在下载内核安装包并执行热替换，请稍候...");
  // 内核下载 + 解压 + 热替换耗时较长，显式传 10 分钟大超时（默认 180s 不够）
  try {
    const res = await invokeWithTimeout<import("@/types").ApiResponse<void>>(
      "core_upgrade_singbox",
      { downloadUrl: updateInfo.value.download_url },
      600000
    );
    if (res.success) {
      toast.success("Sing-box 内核升级成功！", "新版本已自动替换并重新拉起运行");
      await fetchCurrentVersion();
      updateInfo.value.has_update = false;
    } else {
      toast.error("内核升级失败", res.error || "下载或解压过程中发生异常");
    }
  } catch (e) {
    toast.error("内核升级失败", e instanceof Error ? e.message : String(e));
  } finally {
    upgrading.value = false;
  }
}

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("高级设置已保存");
}

async function handlePerfModeChange() {
  await save();
  if (settingsStore.settings.performance_mode) {
    document.documentElement.setAttribute("data-perf-mode", "reduced");
    toast.info("已开启性能模式", "装饰性动画与毛玻璃滤镜已一键关闭");
  } else {
    document.documentElement.removeAttribute("data-perf-mode");
    toast.info("已关闭性能模式", "动效与毛玻璃滤镜已恢复");
  }
}

async function handleRestore() {
  const confirmed = await useConfirm().ask({
    title: "恢复配置备份",
    message: "确定要恢复上次的配置备份 (config.backup.json) 并重启核心吗？",
    confirmText: "恢复并重启",
    level: "danger",
  });
  if (!confirmed) {
    return;
  }
  restoring.value = true;
  toast.info("正在恢复配置文件并重启核心...");
  try {
    const res = await restoreConfigBackup();
    if (res.success) {
      toast.success("配置备份已成功恢复", "内核已重新加载运行");
    } else {
      toast.error("恢复备份失败", res.error);
    }
  } catch (e) {
    toast.error("恢复备份失败", e instanceof Error ? e.message : String(e));
  } finally {
    restoring.value = false;
  }
}

async function handleExportSchema() {
  if (exportingSchema.value) return;
  exportingSchema.value = true;
  toast.info("正在生成 JSON Schema 文件...");
  try {
    const res = await exportConfigSchema();
    if (res.success && res.data) {
      toast.success(
        "JSON Schema 已导出",
        `文件位于 ${res.data}，VS Code 中将 config.json 关联此文件即可获得字段级校验`
      );
    } else {
      toast.error("导出 Schema 失败", res.error || "内核 schema 子命令执行失败");
    }
  } catch (e) {
    toast.error("导出 Schema 失败", e instanceof Error ? e.message : String(e));
  } finally {
    exportingSchema.value = false;
  }
}

// 深链高亮：watch prop 而非在 onMounted 里读一次。
// 父组件 SettingsView 用 watch(immediate) 设置 activePanel，本面板是被
// v-else-if 切换出来的，挂载时机晚于父组件 onMounted；onMounted 里读
// 只能拿到初始空值，高亮永远不触发。
watch(
  () => props.highlightTarget,
  (target) => {
    if (target === "topology") {
      highlightTopology.value = true;
      setTimeout(() => {
        highlightTopology.value = false;
      }, 3000);
    }
  },
  { immediate: true }
);

onMounted(() => {
  fetchCurrentVersion();
});
</script>

<style scoped>
/* 面板骨架（.panel-container / .setting-card / .card-* / .setting-item /
   .item-label / .sub-label）统一走 panel.css 全局定义，
   此处只保留本面板独有的卡片变体与控件样式。 */

.setting-card.highlight-border {
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 25%, transparent);
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent-cyan-vivid) 3%, transparent) 0%, var(--surface-inset) 100%);
}

.btn-check-update {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  border-radius: 8px;
  color: var(--accent-cyan-vivid);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-check-update:hover:not(:disabled) {
  background: var(--accent-cyan-vivid);
  color: var(--text-on-cyan-grad);
}

.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.kernel-status-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: var(--surface-inset);
  border-radius: 8px;
}

.status-left {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
}

.status-label {
  color: var(--text-secondary);
}

.version-badge.current {
  padding: 2px 8px;
  background: color-mix(in srgb, var(--accent-cyan-vivid) 15%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 35%, transparent);
  border-radius: 6px;
  color: var(--accent-cyan-vivid);
  font-weight: 700;
  font-family: monospace;
}

.status-text {
  font-size: 11px;
  color: var(--text-tertiary);
}

.update-found-badge {
  padding: 3px 8px;
  background: color-mix(in srgb, var(--accent-red) 15%, transparent);
  border: 1px solid rgba(239, 68, 68, 0.35);
  border-radius: 6px;
  color: var(--status-danger);
  font-size: 11.5px;
  font-weight: 600;
}

.up-to-date-badge {
  color: var(--accent-green);
  font-size: 12px;
  font-weight: 600;
}

.update-release-box {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 4%, transparent);
  border: 1px dashed color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  border-radius: 10px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.release-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.release-title {
  display: flex;
  align-items: center;
  gap: 10px;
}

.release-tag {
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary);
  font-family: monospace;
}

.release-date {
  font-size: 11px;
  color: var(--text-tertiary);
}

.btn-upgrade-now {
  padding: 7px 16px;
  background: var(--accent-cyan-vivid);
  color: var(--text-on-cyan-grad);
  border: none;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-upgrade-now:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 0 15px color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
}

.btn-upgrade-now:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.upgrading-state {
  display: flex;
  align-items: center;
  gap: 6px;
}

.release-notes {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.notes-title {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--text-secondary);
}

.notes-content {
  font-size: 11px;
  font-family: monospace;
  color: var(--text-secondary);
  background: var(--layer-0);
  padding: 8px 10px;
  border-radius: 6px;
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
  margin: 0;
}

.btn-restore {
  padding: 6px 12px;
  background: var(--surface-hover);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  cursor: pointer;
}

.btn-restore:hover:not(:disabled) {
  background: var(--surface-hover);
}

/* 解锁判据说明块 */
.marker-hint {
  font-size: 11px;
  line-height: 1.6;
  color: var(--text-tertiary);
  padding: 8px 12px;
  background: var(--surface-inset);
  border-left: 2px solid color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
  border-radius: 0 8px 8px 0;
}

/* schema 导出行与内核状态行间距对齐 */
.schema-export-row {
  margin-top: 2px;
}

/* switch 统一走 App.vue 全局胶囊开关（36×20，勾选青色高亮） */
</style>
