<script setup lang="ts">
/**
 * 配置编辑器弹窗（plan-Q Q2）
 * 作者: TanXiang
 *
 * textarea 平文本编辑 + 保存前双重校验（L1 JSON 语法 / L3 sing-box check）：
 * check 不过绝不写回（后端硬边界），通过后备份旧配置并原子写入。
 * 写入后不自动重启内核——「重启内核生效」为独立确认按钮。
 */
import { ref, watch, nextTick } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useToast } from "@/composables/useToast";
import { useConfirm } from "@/composables/useConfirm";
import {
  loadConfigEditor,
  saveConfigEditor,
  restartCoreForEditor,
  type ConfigEditorSaveResult,
} from "@/api/ipc/configEditor";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ close: [] }>();

const toast = useToast();
const confirm = useConfirm();

const content = ref("");
const contentSize = ref(0);
const configPath = ref("");
const loading = ref(false);
const saving = ref(false);
const restarting = ref(false);
const saved = ref(false);
const errorLine = ref<number | null>(null);
const errorMsg = ref("");
const errorLocation = ref("");
const rawOutput = ref("");
const textareaRef = ref<HTMLTextAreaElement | null>(null);

async function fetchContent() {
  loading.value = true;
  try {
    const res = await loadConfigEditor();
    if (res.success && res.data) {
      content.value = res.data.content;
      contentSize.value = res.data.size;
      configPath.value = res.data.path;
      saved.value = false;
      clearError();
    } else {
      toast.error("加载配置失败", res.error || "config.json 读取失败");
    }
  } catch (e) {
    toast.error("加载配置失败", e instanceof Error ? e.message : String(e));
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.visible,
  (v) => {
    if (v) fetchContent();
  }
);

function clearError() {
  errorLine.value = null;
  errorMsg.value = "";
  errorLocation.value = "";
  rawOutput.value = "";
}

function showError(result: Extract<ConfigEditorSaveResult, { status: "syntax_blocked" | "check_blocked" }>) {
  if (result.status === "syntax_blocked") {
    errorLine.value = result.line;
    errorMsg.value = `JSON 语法错误: ${result.message}`;
  } else {
    errorLocation.value = result.location;
    errorMsg.value = result.message || result.raw_output;
    rawOutput.value = result.raw_output;
  }
  // 重新打开后错误面板可能需要滚动到可见区域
  nextTick(() => {
    document.querySelector(".error-panel")?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  });
}

async function handleSave() {
  if (saving.value) return;
  saving.value = true;
  clearError();
  try {
    const res = await saveConfigEditor(content.value);
    if (!res.success || !res.data) {
      toast.error("保存失败", res.error || "校验进程异常");
      return;
    }
    const result: ConfigEditorSaveResult = res.data;
    if (result.status === "syntax_blocked") {
      showError(result);
      toast.error("JSON 语法错误，已拦截保存", `第 ${result.line} 行 第 ${result.column} 列`);
    } else if (result.status === "check_blocked") {
      showError(result);
      toast.error("内核校验未通过，已拦截保存", result.location || "见错误面板");
    } else {
      saved.value = true;
      contentSize.value = result.size;
      toast.success("配置已保存", "改动前内容已备份至 config.backup.json，重启内核后生效");
    }
  } catch (e) {
    toast.error("保存失败", e instanceof Error ? e.message : String(e));
  } finally {
    saving.value = false;
  }
}

async function handleRestartCore() {
  if (restarting.value) return;
  const confirmed = await confirm.ask({
    title: "重启内核",
    message: "确定要立即重启内核使编辑后的配置生效吗？",
    confirmText: "重启内核",
    level: "normal",
  });
  if (!confirmed) return;
  restarting.value = true;
  try {
    const res = await restartCoreForEditor();
    if (res.success) {
      toast.success("内核已重启", "编辑后的配置已生效");
      saved.value = false;
      emit("close");
    } else {
      toast.error("重启内核失败", res.error || "请查看内核日志排查");
    }
  } catch (e) {
    toast.error("重启内核失败", e instanceof Error ? e.message : String(e));
  } finally {
    restarting.value = false;
  }
}

/** Tab 键插入两个空格（纯体验项，防止焦点跳出 textarea） */
function handleTabKey(e: KeyboardEvent) {
  if (e.key !== "Tab") return;
  e.preventDefault();
  const el = e.target as HTMLTextAreaElement;
  const start = el.selectionStart;
  const end = el.selectionEnd;
  content.value = content.value.slice(0, start) + "  " + content.value.slice(end);
  nextTick(() => {
    el.selectionStart = el.selectionEnd = start + 2;
  });
}

function sizeLabel(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.round(bytes / 1024)} KB`;
}
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop" @click.self="emit('close')">
      <div class="modal-card glass-effect editor-modal">
        <div class="modal-header">
          <h3>编辑当前内核配置</h3>
          <button class="close-x" @click="emit('close')">×</button>
        </div>

        <!-- 临时修改预警条（plan-Q A.4-2，固定顶部） -->
        <div class="overwrite-warn">
          <BaseIcon name="AlertTriangle" :size="14" />
          <span>这是对当前生成配置的临时修改，任何订阅刷新或设置变更都会重新生成并覆盖此文件</span>
        </div>

        <div class="modal-body editor-body">
          <div v-if="loading" class="loading-hint">正在加载 config.json ...</div>
          <div v-else class="meta-row">
            <span class="meta-path">{{ configPath }}</span>
            <span class="meta-size" :class="{ large: contentSize > 1024 * 1024 }">
              {{ sizeLabel(contentSize) }}
              <template v-if="contentSize > 1024 * 1024">（大配置，编辑体验可能受限）</template>
            </span>
          </div>
          <textarea
            ref="textareaRef"
            v-model="content"
            class="config-textarea"
            spellcheck="false"
            :disabled="loading"
            @keydown="handleTabKey"
          ></textarea>

          <!-- 错误面板：校验被拦时显示定位 + 原因 -->
          <div v-if="errorMsg" class="error-panel">
            <div class="error-title">
              <BaseIcon name="XCircle" :size="14" />
              <span v-if="errorLine !== null">第 {{ errorLine }} 行 JSON 语法错误</span>
              <span v-else-if="errorLocation">校验未通过：{{ errorLocation }}</span>
              <span v-else>校验未通过</span>
            </div>
            <div class="error-message">{{ errorMsg }}</div>
            <pre v-if="rawOutput" class="error-raw">{{ rawOutput }}</pre>
          </div>
        </div>

        <div class="modal-actions">
          <button class="btn text" @click="emit('close')">取消</button>
          <button class="btn primary" :disabled="saving || loading" @click="handleSave">
            <span v-if="saving" class="spinner"></span>
            {{ saving ? "校验中..." : "校验并保存" }}
          </button>
          <button
            class="btn warning"
            :disabled="!saved || restarting"
            :title="saved ? '' : '请先成功保存修改'"
            @click="handleRestartCore"
          >
            <span v-if="restarting" class="spinner"></span>
            {{ restarting ? "重启中..." : "重启内核生效" }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.editor-modal {
  width: min(860px, 92vw);
  max-height: 86vh;
  display: flex;
  flex-direction: column;
}

.close-x {
  position: absolute;
  right: 0;
  background: transparent;
  border: 0;
  color: var(--text-tertiary);
  font-size: var(--text-xl);
  cursor: pointer;
}

.close-x:hover {
  color: var(--text-primary);
}

.overwrite-warn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  background: rgba(245, 158, 11, 0.08);
  border: 1px solid rgba(245, 158, 11, 0.25);
  color: #fbbf24;
  font-size: var(--text-xs);
  line-height: 1.5;
  margin-bottom: var(--space-2);
}

.editor-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-height: 0;
  flex: 1;
}

.loading-hint {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  padding: var(--space-2);
}

.meta-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  font-size: var(--text-xs);
}

.meta-path {
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta-size {
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.meta-size.large {
  color: #fbbf24;
}

.config-textarea {
  flex: 1;
  min-height: 300px;
  resize: none;
  padding: var(--space-3);
  background: var(--surface-inset);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: var(--leading-normal);
  white-space: pre;
  overflow: auto;
  tab-size: 2;
  outline: none;
}

.config-textarea:focus {
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 50%, transparent);
}

.error-panel {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  background: rgba(239, 68, 68, 0.08);
  border: 1px solid rgba(239, 68, 68, 0.3);
}

.error-title {
  display: flex;
  align-items: center;
  gap: 6px;
  color: #f87171;
  font-size: var(--text-sm);
  font-weight: 600;
}

.error-message {
  color: #fca5a5;
  font-size: var(--text-xs);
  font-family: var(--font-mono);
  word-break: break-all;
}

.error-raw {
  margin: 0;
  max-height: 90px;
  overflow-y: auto;
  padding: 6px 8px;
  background: rgba(0, 0, 0, 0.35);
  border-radius: 4px;
  color: rgba(255, 255, 255, 0.55);
  font-family: var(--font-mono);
  font-size: 10.5px;
  white-space: pre-wrap;
  word-break: break-all;
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

.btn.primary {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.btn.warning {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: rgba(245, 158, 11, 0.12);
  border: 1px solid rgba(245, 158, 11, 0.35);
  color: #fbbf24;
}

.btn.warning:hover:not(:disabled) {
  background: rgba(245, 158, 11, 0.22);
}

.btn.warning:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.spinner {
  width: 12px;
  height: 12px;
  border: 2px solid currentColor;
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  display: inline-block;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
