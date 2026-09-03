<script setup lang="ts">
/**
 * 订阅属性编辑与高级配置模态框
 * 作者: TanXiang
 *
 * 功能：
 * - 基础属性：别名、URL、自定义 UA、后台自动更新周期
 * - 规则清洗：排除关键词/正则过滤、保留节点正则过滤、正则批量重命名
 */
import { ref, computed, watch, onMounted } from "vue";
import BaseIcon from "@/components/common/BaseIcon.vue";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { useToast } from "@/composables/useToast";
import { getSingboxVersion } from "@/api/ipc/proxy";
import type { Subscription } from "@/types";

const props = defineProps<{
  visible: boolean;
  subscription: Subscription | null;
}>();

const emit = defineEmits<{
  (e: "update:visible", val: boolean): void;
  (e: "success"): void;
}>();

const subStore = useSubscriptionStore();
const toast = useToast();

type TabKey = "general" | "filter";
const activeTab = ref<TabKey>("general");

const DEFAULT_UA = "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Mobile Safari/537.36";
const currentSingboxVer = ref("1.14.0");

onMounted(async () => {
  try {
    const res = await getSingboxVersion();
    if (res.success && res.data) {
      currentSingboxVer.value = res.data;
    }
  } catch (_) {}
});

const editName = ref("");
const editUrl = ref("");
const editUa = ref("");
const autoUpdateHours = ref<number>(12);

// 过滤规则
const excludePattern = ref("");
const includePattern = ref("");
const renamePattern = ref("");
const renameReplace = ref("");

const isSaving = ref(false);

const uaPresets = computed(() => [
  { label: "默认 (Pixel 9 / Chrome 151 移动端)", value: DEFAULT_UA },
  { label: "Clash.Meta", value: "clash.meta/v1.18.10" },
  { label: `sing-box 官方 (v${currentSingboxVer.value})`, value: `sing-box/${currentSingboxVer.value}` },
  { label: "v2rayN", value: "v2rayN/6.23" },
  { label: "Quantumult X", value: "Quantumult%20X/1.0.30" },
  { label: "Surge 5", value: "Surge/2800" },
]);

watch(
  () => props.subscription,
  (sub) => {
    if (sub) {
      editName.value = sub.name;
      editUrl.value = sub.url;
      editUa.value = sub.user_agent || DEFAULT_UA;
      autoUpdateHours.value = sub.auto_update_interval_hours ?? 12;

      excludePattern.value = sub.filter_rule?.exclude_pattern || "";
      includePattern.value = sub.filter_rule?.include_pattern || "";
      renamePattern.value = sub.filter_rule?.rename_pattern || "";
      renameReplace.value = sub.filter_rule?.rename_replace || "";
      activeTab.value = "general";
    }
  },
  { immediate: true }
);

function applyUaPreset(val: string) {
  editUa.value = val;
}

function applyExcludePreset(pattern: string) {
  excludePattern.value = pattern;
}

async function handleSave() {
  if (!props.subscription) return;
  if (!editName.value.trim()) {
    toast.warning("订阅别名不能为空");
    return;
  }
  isSaving.value = true;
  try {
    const filterRule = {
      exclude_pattern: excludePattern.value.trim() || undefined,
      include_pattern: includePattern.value.trim() || undefined,
      rename_pattern: renamePattern.value.trim() || undefined,
      rename_replace: renameReplace.value.trim() || undefined,
    };

    // 剪贴板订阅没有 URL，payload 不携带 url 字段，避免把空串误提交覆盖原值
    const isClipboard = props.subscription.source_type === "clipboard";
    const res = await subStore.updateSubMeta(props.subscription.id, {
      name: editName.value.trim(),
      url: isClipboard ? undefined : editUrl.value.trim(),
      userAgent: editUa.value.trim() || undefined,
      autoUpdateIntervalHours: autoUpdateHours.value,
      filterRule: (filterRule.exclude_pattern || filterRule.include_pattern || filterRule.rename_pattern)
        ? filterRule
        : undefined,
    });

    if (res.success) {
      toast.success("订阅属性与清洗规则已保存");
      emit("update:visible", false);
      emit("success");
    } else {
      toast.error("保存失败", res.error || "未知异常");
    }
  } finally {
    isSaving.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="visible && subscription" class="modal-backdrop" @click.self="emit('update:visible', false)">
        <div class="edit-modal glass-effect">
          <!-- 头部 -->
          <div class="modal-header">
            <div class="modal-title">
              <BaseIcon name="Settings2" :size="18" color="#00f2fe" />
              <h3>配置订阅属性</h3>
            </div>
            <button class="btn-close" @click="emit('update:visible', false)">
              <BaseIcon name="X" :size="16" />
            </button>
          </div>

          <!-- 选项卡 Tabs -->
          <div class="modal-tabs">
            <button
              class="tab-btn"
              :class="{ active: activeTab === 'general' }"
              @click="activeTab = 'general'"
            >
              <BaseIcon name="Sliders" :size="13" />
              <span>常规设置</span>
            </button>
            <button
              class="tab-btn"
              :class="{ active: activeTab === 'filter' }"
              @click="activeTab = 'filter'"
            >
              <BaseIcon name="Wrench" :size="13" />
              <span>节点清洗与正则</span>
            </button>
          </div>

          <div class="modal-body">
            <!-- 选项卡 1：常规设置 -->
            <template v-if="activeTab === 'general'">
              <!-- 订阅别名 -->
              <div class="form-group">
                <label>订阅别名</label>
                <input
                  v-model="editName"
                  type="text"
                  class="form-input"
                  placeholder="例如: 主力专线"
                />
              </div>

              <!-- 订阅 URL -->
              <div class="form-group" v-if="subscription.source_type !== 'clipboard'">
                <label>订阅链接 URL</label>
                <input
                  v-model="editUrl"
                  type="text"
                  class="form-input"
                  placeholder="https://..."
                />
              </div>

              <!-- 自定义 User-Agent -->
              <div class="form-group" v-if="subscription.source_type !== 'clipboard'">
                <div class="label-with-hint">
                  <label>自定义请求 User-Agent (UA)</label>
                  <span class="sub-hint">可绕过特定客户端限制</span>
                </div>
                <input
                  v-model="editUa"
                  type="text"
                  class="form-input"
                  placeholder="ClashMeta/1.18.0"
                />
                <div class="preset-chips">
                  <button
                    v-for="p in uaPresets"
                    :key="p.value"
                    class="chip-btn"
                    :class="{ active: editUa === p.value }"
                    @click="applyUaPreset(p.value)"
                  >
                    {{ p.label }}
                  </button>
                </div>
              </div>

              <!-- 自动更新周期 -->
              <div class="form-group" v-if="subscription.source_type !== 'clipboard'">
                <label>后台自动静默更新周期</label>
                <div class="interval-select-row">
                  <button
                    class="interval-btn"
                    :class="{ active: autoUpdateHours === 0 }"
                    @click="autoUpdateHours = 0"
                  >
                    关闭
                  </button>
                  <button
                    class="interval-btn"
                    :class="{ active: autoUpdateHours === 6 }"
                    @click="autoUpdateHours = 6"
                  >
                    每 6 小时
                  </button>
                  <button
                    class="interval-btn"
                    :class="{ active: autoUpdateHours === 12 }"
                    @click="autoUpdateHours = 12"
                  >
                    每 12 小时
                  </button>
                  <button
                    class="interval-btn"
                    :class="{ active: autoUpdateHours === 24 }"
                    @click="autoUpdateHours = 24"
                  >
                    每 24 小时
                  </button>
                </div>
              </div>
            </template>

            <!-- 选项卡 2：节点清洗与正则规则 -->
            <template v-else-if="activeTab === 'filter'">
              <div class="form-group">
                <div class="label-with-hint">
                  <label>排除节点正则表达式 (Exclude)</label>
                  <button
                    class="btn-text-action"
                    @click="applyExcludePreset('官网|重置|流量|到期|剩余|客服|倍率|更新|频道')"
                  >
                    填入常见广告过滤词
                  </button>
                </div>
                <input
                  v-model="excludePattern"
                  type="text"
                  class="form-input"
                  placeholder="例如: 官网|重置|流量|客服|倍率"
                />
                <span class="field-hint">匹配该正则表达式的节点名称将被自动剔除</span>
              </div>

              <div class="form-group">
                <label>保留节点正则表达式 (Include)</label>
                <input
                  v-model="includePattern"
                  type="text"
                  class="form-input"
                  placeholder="例如: 香港|日本|新加坡|美国|HK|JP|SG|US"
                />
                <span class="field-hint">仅保留符合该模式的节点（留空则保留所有非排除节点）</span>
              </div>

              <div class="form-group">
                <label>节点批量正则重命名 (Rename)</label>
                <div class="rename-row">
                  <input
                    v-model="renamePattern"
                    type="text"
                    class="form-input"
                    placeholder="查找正则表达式，如: 【.*?】"
                  />
                  <span class="arrow-sep">➔</span>
                  <input
                    v-model="renameReplace"
                    type="text"
                    class="form-input"
                    placeholder="替换文本 (可为空)"
                  />
                </div>
                <span class="field-hint">可在下次更新或应用时自动格式化节点命名</span>
              </div>
            </template>
          </div>

          <div class="modal-footer">
            <button class="btn-cancel" @click="emit('update:visible', false)">取消</button>
            <button class="btn-submit" :disabled="isSaving" @click="handleSave">
              <BaseIcon v-if="!isSaving" name="Check" :size="14" />
              <span>{{ isSaving ? "正在保存..." : "保存更改" }}</span>
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

.edit-modal {
  width: 500px;
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

.modal-tabs {
  display: flex;
  padding: 6px 16px;
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
  background: rgba(0, 242, 254, 0.12);
  border-color: rgba(0, 242, 254, 0.25);
  color: #00f2fe;
}

.modal-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 240px;
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

.label-with-hint {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.btn-text-action {
  background: transparent;
  border: none;
  color: #00f2fe;
  font-size: 11px;
  cursor: pointer;
  padding: 0;
}

.btn-text-action:hover {
  text-decoration: underline;
}

.sub-hint {
  font-size: 10.5px;
  color: rgba(255, 255, 255, 0.35);
}

.field-hint {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.35);
}

.form-input {
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

.form-input:focus {
  border-color: #00f2fe;
}

.rename-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.rename-row .form-input {
  flex: 1;
}

.arrow-sep {
  color: rgba(255, 255, 255, 0.4);
  font-size: 13px;
}

.preset-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 4px;
}

.chip-btn {
  padding: 3px 8px;
  border-radius: 5px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.6);
  font-size: 10.5px;
  cursor: pointer;
  transition: all 0.15s ease;
}

.chip-btn:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.08);
}

.chip-btn.active {
  background: rgba(0, 242, 254, 0.15);
  border-color: rgba(0, 242, 254, 0.3);
  color: #00f2fe;
}

.interval-select-row {
  display: flex;
  gap: 8px;
}

.interval-btn {
  flex: 1;
  padding: 7px 10px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.65);
  font-size: 11.5px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.interval-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}

.interval-btn.active {
  background: rgba(0, 242, 254, 0.12);
  border-color: rgba(0, 242, 254, 0.3);
  color: #00f2fe;
  font-weight: 600;
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
  background: linear-gradient(135deg, rgba(0, 242, 254, 0.9) 0%, rgba(79, 172, 254, 0.9) 100%);
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
