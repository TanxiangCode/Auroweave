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
        <button
          class="btn-check-update"
          :disabled="coreStore.checking || coreStore.isUpgrading"
          @click="handleCheckUpdate"
        >
          <span :class="{ spinning: coreStore.checking }"></span>
          <span>{{ coreStore.checking ? '正在检查...' : '检查更新' }}</span>
        </button>
      </div>

      <div class="card-body">
        <div class="kernel-status-row">
          <div class="status-left">
            <span class="status-label">当前运行内核：</span>
            <span class="version-badge current">v{{ coreStore.currentVersion }}</span>
            <span class="status-text">100% 规则对齐 (Official Sidecar)</span>
            <!-- macOS TUN 依赖 SUID root：内核被替换后该属性会丢，
                 这里主动提示，避免用户遇到"TUN 突然起不来"却无从排查 -->
            <span
              v-if="isMac && !coreStore.kernelPrivileged"
              class="suid-warning"
              title="TUN 模式需要内核具备管理员权限。请在「TUN 网卡」面板重新授权"
            >
              <BaseIcon name="AlertTriangle" :size="12" />
              TUN 模式暂不可用（内核缺少管理员权限）
            </span>
          </div>

          <div v-if="coreStore.updateInfo" class="status-right">
            <span v-if="coreStore.updateInfo.has_update" class="update-found-badge">
              发现新版本 {{ coreStore.updateInfo.latest_version }}
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
          <button class="btn-restore" :disabled="exportingSchema" @click="handleExportSchema">
            <span>{{ exportingSchema ? '正在导出...' : '导出 Schema' }}</span>
          </button>
        </div>

        <!-- 升级通知卡片：发现新版本、或升级任务在途/刚结束时显示。
             升级中即便 updateInfo 因切页丢失也必须常驻，否则进度会凭空消失 -->
        <div v-if="showUpdateBox" class="update-release-box">
          <div class="release-header">
            <div class="release-title">
              <span v-if="latestVersion" class="release-tag">{{ latestVersion }}</span>
              <span class="release-date">
                {{ releaseSubtitle }}
              </span>
            </div>
            <button
              class="btn-upgrade-now"
              :class="{ 'is-running': coreStore.isUpgrading }"
              :disabled="coreStore.isUpgrading || !downloadUrl"
              :title="downloadUrl ? '' : '未找到对应平台的下载资产'"
              @click="handleUpgrade"
            >
              <span v-if="!coreStore.isUpgrading">立即在线升级</span>
              <span v-else class="upgrading-state">
                <span class="spinner"></span>
                <span>{{ coreStore.stageLabel }}</span>
              </span>
              <!-- 按钮内进度条：与文字同处一个按钮，切页回来进度即刻可见 -->
              <span v-if="coreStore.isUpgrading" class="btn-progress-track">
                <span class="btn-progress-fill" :style="{ width: coreStore.progress.percent + '%' }"></span>
              </span>
            </button>
          </div>

          <!-- 阶段细节 + 结果态文案 -->
          <div v-if="coreStore.isUpgrading" class="upgrade-progress-detail">
            {{ coreStore.progressDetail }}
          </div>
          <div
            v-else-if="coreStore.isFinished"
            class="upgrade-result"
            :class="coreStore.progress.success ? 'ok' : 'fail'"
          >
            <BaseIcon :name="coreStore.progress.success ? 'Check' : 'X'" :size="13" />
            <span>{{ coreStore.resultMessage }}</span>
          </div>

          <!-- 更新日志摘要 -->
          <div v-if="coreStore.updateInfo?.release_notes" class="release-notes">
            <div class="notes-title">更新日志 (Changelog):</div>
            <pre class="notes-content">{{ coreStore.updateInfo.release_notes }}</pre>
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
            v-model="settingsStore.settings.topology_enabled"
            type="checkbox"
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
            v-model="settingsStore.settings.performance_mode"
            type="checkbox"
            class="switch"
            @change="handlePerfModeChange"
          />
        </div>

        <div class="setting-item">
          <div class="item-label">
            <span>恢复上一次配置备份</span>
            <span class="sub-label">若当前内核配置文件异常，一键恢复 config.backup.json 并重启</span>
          </div>
          <button class="btn-restore" :disabled="restoring" @click="handleRestore">
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
import { useCoreUpdateStore } from "@/stores/coreUpdate.store";
import { isMacOS } from "@/utils/format";
import { useToast } from "@/composables/useToast";
import { useConfirm } from "@/composables/useConfirm";
import { restoreConfigBackup } from "@/api/ipc/settings";
import { exportConfigSchema } from "@/api/ipc/configEditor";
import ConfigEditorModal from "./ConfigEditorModal.vue";

const props = defineProps<{
  highlightTarget?: string;
}>();

const settingsStore = useSettingsStore();
const toast = useToast();
const coreStore = useCoreUpdateStore();
/** SUID/TUN 相关提示仅 macOS 适用：Windows 用服务、Linux 无 SUID 概念 */
const isMac = ref(isMacOS());

const highlightTopology = ref(false);

/** test-core 端口基址：0 / 未设置时回落到后端内置默认 40040，仅用于文案展示 */
const testCorePortBase = computed(() => settingsStore.settings.test_core_port_base || 0);

/** 新版本号（无 updateInfo 时降级为空串，避免渲染出空的版本标签） */
const latestVersion = computed(() => coreStore.updateInfo?.latest_version || "");
/** 下载资产地址：缺失时升级按钮禁用并给出 title 提示 */
const downloadUrl = computed(() => coreStore.updateInfo?.download_url || "");
/** 升级卡片副标题：升级中优先显示"目标版本"，否则显示发布时间 */
const releaseSubtitle = computed(() => {
  if (coreStore.isUpgrading) return "正在升级到该版本";
  if (!coreStore.updateInfo?.published_at) return "";
  return `发布于 ${formatDate(coreStore.updateInfo.published_at)}`;
});
/**
 * 升级卡片可见性
 *
 * 关键：任务在途（isUpgrading）或刚结束（isFinished）时必须保持显示。
 * 只按 has_update 判断的话，切页导致 updateInfo 丢失后卡片会整体消失，
 * 用户既看不到进度也点不了重试。
 */
const showUpdateBox = computed(
  () => coreStore.isUpgrading || coreStore.isFinished || !!coreStore.updateInfo?.has_update
);

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
const restoring = ref(false);
const exportingSchema = ref(false);
const showConfigEditor = ref(false);

function formatDate(dateStr: string): string {
  if (!dateStr) return "";
  try {
    const d = new Date(dateStr);
    return d.toLocaleDateString("zh-CN", { year: "numeric", month: "long", day: "numeric" });
  } catch {
    return dateStr;
  }
}

// 检查更新 / 在线升级的逻辑与状态全部下沉到 coreUpdate.store：
// 组件只负责渲染与转发点击，进度状态不随本组件卸载而消失。
function handleCheckUpdate() {
  coreStore.clearResult();
  void coreStore.checkUpdate();
}

function handleUpgrade() {
  if (!downloadUrl.value) {
    toast.error("未找到对应平台的下载资产");
    return;
  }
  void coreStore.startUpgrade(downloadUrl.value);
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

onMounted(async () => {
  // init() 幂等：App.vue 启动时已注册监听并回查过后端进度，
  // 这里再调一次只为在深链直达本面板时也能确保状态就绪
  await coreStore.init();
  await coreStore.fetchCurrentVersion();
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

/* 内核缺少 SUID 权限告警：仅 macOS 出现，提示 TUN 模式不可用 */
.suid-warning {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  background: color-mix(in srgb, var(--status-danger) 12%, transparent);
  border: 1px solid color-mix(in srgb, var(--status-danger) 35%, transparent);
  border-radius: 6px;
  color: var(--status-danger);
  font-size: 11px;
  font-weight: 600;
  cursor: help;
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
  /* 升级中按钮内要容纳"文字 + 进度条"两行，故允许换行并锁定最小宽度，
     避免百分比数字跳动导致按钮宽度反复重排 */
  position: relative;
  display: inline-flex;
  flex-direction: column;
  align-items: stretch;
  gap: 5px;
  min-width: 148px;
  padding: 7px 16px;
  background: var(--accent-cyan-vivid);
  color: var(--text-on-cyan-grad);
  border: none;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
  transition: all 0.15s;
  overflow: hidden;
  text-align: center;
}

.btn-upgrade-now:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 0 15px color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
}

.btn-upgrade-now:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* 升级中：半透明底 + 禁止 hover 位移，视觉上与"可点击"区分开 */
.btn-upgrade-now.is-running {
  transform: none;
  background: color-mix(in srgb, var(--accent-cyan-vivid) 22%, transparent);
  color: var(--accent-cyan-vivid);
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent-cyan-vivid) 45%, transparent);
}

/* 按钮内进度条：随百分比推进，是"进度显示在按钮上"的落点 */
.btn-progress-track {
  display: block;
  width: 100%;
  height: 4px;
  background: color-mix(in srgb, var(--text-on-cyan-grad) 25%, transparent);
  border-radius: var(--radius-full, 9999px);
  overflow: hidden;
}

.btn-progress-fill {
  display: block;
  height: 100%;
  background: var(--text-on-cyan-grad);
  border-radius: var(--radius-full, 9999px);
  transition: width 0.2s ease-out;
}

/* 升级中按钮内的加载圈（此前模板用了 .spinner 但面板未定义样式，实际不可见） */
.spinner {
  display: inline-block;
  width: 11px;
  height: 11px;
  border: 2px solid currentColor;
  border-right-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  flex-shrink: 0;
}

.upgrading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  white-space: nowrap;
}

/* 进度条下方的阶段细节文案（已下载 / 总量等） */
.upgrade-progress-detail {
  font-size: 11px;
  color: var(--text-secondary);
  font-family: monospace;
  word-break: break-all;
}

/* 升级结果态（成功绿 / 失败红） */
.upgrade-result {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  font-weight: 600;
}

.upgrade-result.ok {
  color: var(--accent-green);
}

.upgrade-result.fail {
  color: var(--status-danger);
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
