<script setup lang="ts">
/**
 * 订阅多源导入模态框
 * 作者: TanXiang
 *
 * 支持三种导入模式：
 * 1. 远程 URL 导入 (Clash / sing-box / V2Ray 订阅链接)
 * 2. 剪贴板快速导入 (一键读取并聚合 vmess://, vless://, ss://, trojan://, hysteria2:// 或配置文本)
 * 3. 本地文件导入 (支持选择或拖拽 .yaml, .json, .txt 配置文件)
 */
import { ref, watch } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { useToast } from "@/composables/useToast";

const props = defineProps<{
  visible: boolean;
}>();

const emit = defineEmits<{
  (e: "update:visible", val: boolean): void;
  (e: "success"): void;
}>();

const subStore = useSubscriptionStore();
const toast = useToast();

type ImportMode = "url" | "clipboard" | "file";
const activeMode = ref<ImportMode>("url");

const subName = ref("");
const subUrl = ref("");
const clipboardText = ref("");
const localFileName = ref("");
const localFileContent = ref("");
const autoGroup = ref(true);
const isSubmitting = ref(false);

watch(
  () => props.visible,
  (val) => {
    if (val) {
      subName.value = "";
      subUrl.value = "";
      clipboardText.value = "";
      localFileName.value = "";
      localFileContent.value = "";
      activeMode.value = "url";
    }
  }
);

/** 从系统剪贴板快速读取 */
async function handleReadClipboard() {
  try {
    const text = await navigator.clipboard.readText();
    if (text && text.trim()) {
      clipboardText.value = text.trim();
      toast.success("已从剪贴板读取内容", `共 ${text.length} 字符`);
      if (!subName.value) {
        subName.value = "剪贴板节点集合";
      }
    } else {
      toast.warning("剪贴板中无有效文本内容");
    }
  } catch (e) {
    toast.error("读取剪贴板失败", "请手动在输入框中粘贴内容");
  }
}

/** 选择本地文件 */
function handleFileChange(event: Event) {
  const target = event.target as HTMLInputElement;
  if (!target.files || target.files.length === 0) return;
  const file = target.files[0];
  localFileName.value = file.name;
  if (!subName.value) {
    subName.value = file.name.replace(/\.[^/.]+$/, "");
  }

  const reader = new FileReader();
  reader.onload = (e) => {
    localFileContent.value = (e.target?.result as string) || "";
    toast.success("文件读取成功", `${file.name} (${Math.round(file.size / 1024)} KB)`);
  };
  reader.onerror = () => {
    toast.error("读取文件失败", "请确认文件编码为 UTF-8");
  };
  reader.readAsText(file);
}

/** 提交导入 */
async function handleSubmit() {
  isSubmitting.value = true;
  try {
    if (activeMode.value === "url") {
      if (!subUrl.value.trim()) {
        toast.warning("请输入订阅 URL");
        isSubmitting.value = false;
        return;
      }
      const name = subName.value.trim() || `订阅_${subStore.subscriptions.length + 1}`;
      const res = await subStore.importSub(name, subUrl.value.trim(), autoGroup.value);
      if (res.success) {
        toast.success("远程订阅导入成功", `载入 ${res.data?.node_count || 0} 个节点`);
        emit("update:visible", false);
        emit("success");
      } else {
        toast.error("导入失败", res.error || "请检查网络或订阅链接");
      }
    } else if (activeMode.value === "clipboard") {
      if (!clipboardText.value.trim()) {
        toast.warning("请先粘贴或读取节点内容");
        isSubmitting.value = false;
        return;
      }
      const name = subName.value.trim() || "剪贴板节点源";
      const res = await subStore.importContentSub(
        name,
        clipboardText.value.trim(),
        "clipboard",
        undefined,
        autoGroup.value
      );
      if (res.success) {
        toast.success("剪贴板节点导入成功", `载入 ${res.data?.node_count || 0} 个节点`);
        emit("update:visible", false);
        emit("success");
      } else {
        toast.error("解析失败", res.error || "未识别到有效的节点链接");
      }
    } else if (activeMode.value === "file") {
      if (!localFileContent.value.trim()) {
        toast.warning("请先选择本地配置文件");
        isSubmitting.value = false;
        return;
      }
      const name = subName.value.trim() || localFileName.value || "本地配置源";
      const res = await subStore.importContentSub(
        name,
        localFileContent.value.trim(),
        "local_file",
        localFileName.value,
        autoGroup.value
      );
      if (res.success) {
        toast.success("本地文件导入成功", `载入 ${res.data?.node_count || 0} 个节点`);
        emit("update:visible", false);
        emit("success");
      } else {
        toast.error("解析文件失败", res.error || "文件格式无法解析为合法节点");
      }
    }
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="visible" class="modal-backdrop">
        <div class="import-modal glass-effect">
          <!-- 弹窗头部 -->
          <div class="modal-header">
            <div class="modal-title">
              <BaseIcon name="PlusCircle" :size="18" color="var(--accent-cyan-vivid)" />
              <h3>多源订阅导入</h3>
            </div>
            <button class="btn-close" @click="emit('update:visible', false)">
              <BaseIcon name="X" :size="16" />
            </button>
          </div>

          <!-- 模式切换 Tabs -->
          <div class="mode-tabs">
            <button
              class="tab-btn"
              :class="{ active: activeMode === 'url' }"
              @click="activeMode = 'url'"
            >
              <BaseIcon name="Globe" :size="13" />
              <span>远程 URL 订阅</span>
            </button>
            <button
              class="tab-btn"
              :class="{ active: activeMode === 'clipboard' }"
              @click="activeMode = 'clipboard'"
            >
              <BaseIcon name="ClipboardList" :size="13" />
              <span>剪贴板批量导入</span>
            </button>
            <button
              class="tab-btn"
              :class="{ active: activeMode === 'file' }"
              @click="activeMode = 'file'"
            >
              <BaseIcon name="FileUp" :size="13" />
              <span>本地文件导入</span>
            </button>
          </div>

          <!-- 表单内容区 -->
          <div class="modal-body">
            <!-- 别名 -->
            <div class="form-group">
              <label>订阅别名</label>
              <input
                v-model="subName"
                type="text"
                placeholder="例如: SKYLUMO 机场 / 专线主力"
                class="form-input"
              />
            </div>

            <!-- Mode 1: 远程 URL -->
            <div v-if="activeMode === 'url'" class="form-group">
              <label>订阅链接 URL <span class="required">*</span></label>
              <input
                v-model="subUrl"
                type="text"
                placeholder="https://airport.com/api/v1/client/subscribe?token=..."
                class="form-input"
                @keydown.enter="handleSubmit"
              />
              <span class="field-hint">支持 Clash YAML、sing-box JSON 与 Base64 订阅源</span>
            </div>

            <!-- Mode 2: 剪贴板 / 批量节点链接 -->
            <div v-else-if="activeMode === 'clipboard'" class="form-group">
              <div class="label-with-action">
                <label>节点链接文本 (单行/多行) <span class="required">*</span></label>
                <button class="btn-text-action" @click="handleReadClipboard">
                  <BaseIcon name="ClipboardList" :size="12" />
                  <span>读取剪贴板</span>
                </button>
              </div>
              <textarea
                v-model="clipboardText"
                rows="4"
                placeholder="直接粘贴 vmess://, vless://, ss://, trojan://, hysteria2:// 或 Base64 文本..."
                class="form-textarea"
              ></textarea>
              <span class="field-hint">系统将自动扫描并提取其中的全部有效节点链接</span>
            </div>

            <!-- Mode 3: 本地配置文件 -->
            <div v-else-if="activeMode === 'file'" class="form-group">
              <label>选择本地配置文件 (.yaml / .json / .txt) <span class="required">*</span></label>
              <div class="file-upload-box">
                <input
                  type="file"
                  accept=".yaml,.yml,.json,.txt"
                  id="sub-file-input"
                  class="file-input-hidden"
                  @change="handleFileChange"
                />
                <label for="sub-file-input" class="file-upload-label">
                  <BaseIcon name="FileUp" :size="24" color="var(--accent-cyan-vivid)" />
                  <span v-if="localFileName" class="selected-filename">{{ localFileName }}</span>
                  <span v-else class="upload-guide">点击选择本地配置文件</span>
                </label>
              </div>
            </div>

            <!-- 自动分组开关 -->
            <div class="form-checkbox-row">
              <label class="checkbox-label">
                <input type="checkbox" v-model="autoGroup" />
                <span>自动按地区分类并生成延迟自动优选组 (HK/JP/US 等)</span>
              </label>
            </div>
          </div>

          <!-- 弹窗底部 -->
          <div class="modal-footer">
            <button class="btn-cancel" @click="emit('update:visible', false)">取消</button>
            <button
              class="btn-submit"
              :disabled="isSubmitting"
              @click="handleSubmit"
            >
              <BaseIcon v-if="!isSubmitting" name="Download" :size="14" />
              <span>{{ isSubmitting ? "正在解析导入..." : "开始导入" }}</span>
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.import-modal {
  width: 480px;
  border-radius: 16px;
  background: rgba(18, 22, 30, 0.96);
  border: 1px solid rgba(255, 255, 255, 0.12);
  box-shadow: 0 24px 48px rgba(0, 0, 0, 0.7);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.modal-title {
  display: flex;
  align-items: center;
  gap: 8px;
}

.modal-title h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: #fff;
}

.btn-close {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.4);
  cursor: pointer;
  padding: 4px;
}

.btn-close:hover {
  color: #fff;
}

.mode-tabs {
  display: flex;
  padding: 8px 16px;
  gap: 6px;
  background: rgba(0, 0, 0, 0.25);
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.tab-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 7px 10px;
  border-radius: 8px;
  border: 1px solid transparent;
  background: transparent;
  color: rgba(255, 255, 255, 0.55);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.tab-btn:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.04);
}

.tab-btn.active {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 25%, transparent);
  color: var(--accent-cyan-vivid);
}

.modal-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-group label {
  font-size: 12px;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.75);
}

.required {
  color: var(--status-danger);
}

.label-with-action {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.btn-text-action {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: transparent;
  border: none;
  color: var(--accent-cyan-vivid);
  font-size: 11px;
  cursor: pointer;
}

.btn-text-action:hover {
  text-decoration: underline;
}

.form-input,
.form-textarea {
  padding: 9px 12px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #fff;
  font-size: 12.5px;
  outline: none;
  transition: border-color 0.2s;
  font-family: inherit;
}

.form-textarea {
  resize: vertical;
  line-height: 1.4;
}

.form-input:focus,
.form-textarea:focus {
  border-color: var(--accent-cyan-vivid);
}

.field-hint {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.35);
}

.file-upload-box {
  position: relative;
}

.file-input-hidden {
  display: none;
}

.file-upload-label {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 24px 16px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px dashed rgba(255, 255, 255, 0.15);
  cursor: pointer;
  transition: all 0.2s ease;
}

.file-upload-label:hover {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 5%, transparent);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
}

.upload-guide {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.6);
}

.selected-filename {
  font-size: 12px;
  font-weight: 600;
  color: var(--accent-cyan-vivid);
}

.form-checkbox-row {
  margin-top: 2px;
}

.checkbox-label {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11.5px;
  color: rgba(255, 255, 255, 0.65);
  cursor: pointer;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 20px;
  background: rgba(0, 0, 0, 0.25);
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.btn-cancel {
  padding: 7px 14px;
  border-radius: 8px;
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
  cursor: pointer;
}

.btn-submit {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 16px;
  border-radius: 8px;
  background: linear-gradient(135deg, color-mix(in srgb, var(--accent-cyan-vivid) 90%, transparent) 0%, rgba(79, 172, 254, 0.9) 100%);
  color: #000;
  font-size: 12px;
  font-weight: 600;
  border: none;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn-submit:hover:not(:disabled) {
  filter: brightness(1.1);
}

.btn-submit:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
