<template>
  <div class="custom-rules-container">
    <!-- 头部操作与检索栏 -->
    <div class="rules-toolbar">
      <!-- 搜索框 -->
      <div class="search-box">
        <span class="search-icon"></span>
        <input
          v-model="searchQuery"
          type="text"
          placeholder="搜索域名、后缀、关键字、IP 或备注..."
        />
        <button v-if="searchQuery" class="btn-clear" @click="searchQuery = ''"><BaseIcon name="X" :size="14" /></button>
      </div>

      <!-- 类型胶囊筛选 -->
      <div class="type-capsules">
        <button
          class="capsule"
          :class="{ active: typeFilter === 'all' }"
          @click="typeFilter = 'all'"
        >
          全部 ({{ rules.length }})
        </button>
        <button
          class="capsule domain_suffix"
          :class="{ active: typeFilter === 'domain_suffix' }"
          @click="typeFilter = 'domain_suffix'"
        >
          后缀通配 ({{ countByType('domain_suffix') }})
        </button>
        <button
          class="capsule domain"
          :class="{ active: typeFilter === 'domain' }"
          @click="typeFilter = 'domain'"
        >
          精准域名 ({{ countByType('domain') }})
        </button>
        <button
          class="capsule domain_keyword"
          :class="{ active: typeFilter === 'domain_keyword' }"
          @click="typeFilter = 'domain_keyword'"
        >
          关键字 ({{ countByType('domain_keyword') }})
        </button>
        <button
          class="capsule ip_cidr"
          :class="{ active: typeFilter === 'ip_cidr' }"
          @click="typeFilter = 'ip_cidr'"
        >
          IP 网段 ({{ countByType('ip_cidr') }})
        </button>
      </div>

      <!-- 导入 / 导出 / 新增规则按钮 -->
      <button class="btn-add-rule" title="从 JSON 文件导入分流规则（与现有规则合并去重）" @click="handleImport">
        <span class="btn-add-content"><BaseIcon name="Download" :size="15" /> 导入</span>
      </button>
      <button class="btn-add-rule" title="导出全部分流规则为 JSON 文件" @click="handleExport">
        <span class="btn-add-content"><BaseIcon name="Share2" :size="15" /> 导出</span>
      </button>
      <button class="btn-add-rule primary" @click="openAddModal">
        <span class="btn-add-content"><BaseIcon name="Plus" :size="15" /> 添加分流规则</span>
      </button>
    </div>

    <!-- 导入文件选择（隐藏 input） -->
    <input
      ref="importFileInput"
      type="file"
      accept=".json,application/json"
      style="display: none"
      @change="onImportFileSelected"
    />

    <!-- 规则列表 -->
    <div class="rules-body">
      <div v-if="loading && rules.length === 0" class="state-box glass-effect">
        <span class="spinner"></span>
        <p>正在读取自定义分流策略...</p>
      </div>

      <div v-else-if="filteredRules.length === 0" class="state-box glass-effect">
        <span class="empty-icon"></span>
        <p>{{ rules.length === 0 ? '暂无自定义域名/IP 分流规则，点击上方「添加分流规则」开始配置' : '未找到匹配的规则条目' }}</p>
      </div>

      <div v-else class="rules-list">
        <div
          v-for="rule in filteredRules"
          :key="rule.id"
          class="rule-card glass-effect"
          :class="{ disabled: !rule.enabled }"
        >
          <!-- 左侧：类型标识与匹配表达式 -->
          <div class="rule-left">
            <span class="type-badge" :class="rule.rule_type">
              {{ getTypeLabel(rule.rule_type) }}
            </span>
            <div class="rule-details">
              <span class="payload-text" :title="rule.payload">{{ rule.payload }}</span>
              <span class="desc-text" v-if="rule.description">{{ rule.description }}</span>
            </div>
          </div>

          <!-- 中间：分流流向 -->
          <div class="rule-arrow">
            <span class="arrow-line"></span>
            <span class="arrow-head">→</span>
          </div>

          <!-- 右侧：出站绑定器与操作 -->
          <div class="rule-right">
            <OutboundSelector
              :model-value="rule.outbound_tag"
              @change="val => handleOutboundChange(rule, val)"
            />

            <!-- 启停开关 -->
            <label class="switch-wrap" :title="rule.enabled ? '已启用（点击禁用）' : '已禁用（点击启用）'">
              <input
                type="checkbox"
                class="switch"
                :checked="rule.enabled"
                @change="toggleRuleEnabled(rule)"
              />
            </label>

            <!-- 删除按钮 -->
            <button
              class="btn-delete"
              title="删除此条规则"
              @click="handleDeleteRule(rule.id)"
            >
              
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- 添加/编辑规则模态弹窗 -->
    <div v-if="showModal" class="modal-overlay" @click.self="showModal = false">
      <div class="modal-card glass-effect">
        <div class="modal-header">
          <h3>{{ isEditing ? '编辑分流规则' : '添加自定义分流规则' }}</h3>
          <button class="btn-close" @click="showModal = false"><BaseIcon name="X" :size="14" /></button>
        </div>

        <div class="modal-body">
          <!-- 规则类型选择 -->
          <div class="form-group">
            <label class="form-label">匹配模式类型</label>
            <div class="type-selector-grid">
              <label
                class="type-radio-btn"
                :class="{ active: formRule.rule_type === 'domain_suffix' }"
              >
                <input type="radio" v-model="formRule.rule_type" value="domain_suffix" />
                <span class="radio-title">域名后缀通配</span>
                <span class="radio-sub">例如 .github.com, cn</span>
              </label>

              <label
                class="type-radio-btn"
                :class="{ active: formRule.rule_type === 'domain' }"
              >
                <input type="radio" v-model="formRule.rule_type" value="domain" />
                <span class="radio-title">完整域名精准匹配</span>
                <span class="radio-sub">例如 chatgpt.com</span>
              </label>

              <label
                class="type-radio-btn"
                :class="{ active: formRule.rule_type === 'domain_keyword' }"
              >
                <input type="radio" v-model="formRule.rule_type" value="domain_keyword" />
                <span class="radio-title">域名关键字匹配</span>
                <span class="radio-sub">例如 openai, twitter</span>
              </label>

              <label
                class="type-radio-btn"
                :class="{ active: formRule.rule_type === 'ip_cidr' }"
              >
                <input type="radio" v-model="formRule.rule_type" value="ip_cidr" />
                <span class="radio-title">IP / CIDR 网段</span>
                <span class="radio-sub">例如 1.1.1.1/32</span>
              </label>
            </div>
          </div>

          <!-- 匹配表达式输入 -->
          <div class="form-group">
            <label class="form-label">匹配内容 (Payload)</label>
            <input
              v-model="formRule.payload"
              type="text"
              class="form-input mono"
              :placeholder="getPlaceholder(formRule.rule_type)"
            />
          </div>

          <!-- 目标出站选择 -->
          <div class="form-group">
            <label class="form-label">目标出站策略 / 节点</label>
            <OutboundSelector
              :model-value="formRule.outbound_tag"
              @change="val => formRule.outbound_tag = val"
            />
          </div>

          <!-- 备注说明 -->
          <div class="form-group">
            <label class="form-label">备注说明 (选填)</label>
            <input
              v-model="formRule.description"
              type="text"
              class="form-input"
              placeholder="例如：开发平台专线加速、公司内网直连等"
            />
          </div>
        </div>

        <div class="modal-footer">
          <button class="btn-modal-cancel" @click="showModal = false">取消</button>
          <button class="btn-modal-submit" @click="submitRule" :disabled="!formRule.payload.trim()">
            保存并热重载
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, computed, onMounted } from "vue";
import {
  getCustomRules,
  addCustomRule,
  deleteCustomRule,
  exportRoutingRules,
  importRoutingRules,
  type CustomRuleItem,
} from "@/api/ipc/routing";
import { useToast } from "@/composables/useToast";
import OutboundSelector from "./OutboundSelector.vue";

const rules = ref<CustomRuleItem[]>([]);
const searchQuery = ref("");
const typeFilter = ref<string>("all");
const loading = ref(false);
const showModal = ref(false);
const isEditing = ref(false);
const toast = useToast();
const importFileInput = ref<HTMLInputElement | null>(null);

/** 导出全部规则：落盘到用户选择的文件（应用内拼 JSON，经 Blob 下载） */
async function handleExport() {
  const res = await exportRoutingRules();
  if (!res.success || !res.data) {
    toast.error("导出失败", res.error || "读取规则失败");
    return;
  }
  try {
    const blob = new Blob([res.data], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `auroweave-routing-rules-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success("导出完成", "规则备份已下载");
  } catch (e) {
    toast.error("导出失败", String(e));
  }
}

/** 导入：弹文件选择器，读取 JSON 后合并导入 */
function handleImport() {
  importFileInput.value?.click();
}

async function onImportFileSelected(e: Event) {
  const input = e.target as HTMLInputElement;
  const file = input.files?.[0];
  // 允许同一路径重复选择同一文件
  input.value = "";
  if (!file) return;
  try {
    const text = await file.text();
    const res = await importRoutingRules(text, true);
    if (res.success && res.data) {
      const [imported, skipped] = res.data;
      toast.success("导入完成", `导入 ${imported} 条，跳过 ${skipped} 条`);
      await fetchData();
    } else {
      toast.error("导入失败", res.error || "文件格式不正确");
    }
  } catch (err) {
    toast.error("导入失败", String(err));
  }
}

const formRule = ref<CustomRuleItem>({
  id: "",
  rule_type: "domain_suffix",
  payload: "",
  outbound_tag: "proxy",
  enabled: true,
  description: "",
});

function countByType(type: string): number {
  return rules.value.filter((r) => r.rule_type === type).length;
}

const filteredRules = computed(() => {
  let list = rules.value;

  if (typeFilter.value !== "all") {
    list = list.filter((r) => r.rule_type === typeFilter.value);
  }

  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter(
      (r) =>
        r.payload.toLowerCase().includes(q) ||
        (r.description && r.description.toLowerCase().includes(q)) ||
        r.outbound_tag.toLowerCase().includes(q)
    );
  }

  return list;
});

function getTypeLabel(type: string): string {
  switch (type) {
    case "domain": return "精准域名";
    case "domain_suffix": return "后缀通配";
    case "domain_keyword": return "关键字";
    case "domain_regex": return "正则匹配";
    case "ip_cidr": return "IP 网段";
    default: return "自定义";
  }
}

function getPlaceholder(type: string): string {
  switch (type) {
    case "domain": return "例如 chatgpt.com 或 api.github.com";
    case "domain_suffix": return "例如 .github.com 或 .openai.com 或 cn";
    case "domain_keyword": return "例如 google 或 steam";
    case "domain_regex": return "例如 ^api\..+";
    case "ip_cidr": return "例如 192.168.1.0/24 或 1.1.1.1/32";
    default: return "请输入匹配表达式";
  }
}

async function fetchData() {
  loading.value = true;
  const res = await getCustomRules();
  if (res.success && res.data) {
    rules.value = res.data;
  }
  loading.value = false;
}

function openAddModal() {
  isEditing.value = false;
  formRule.value = {
    id: Date.now().toString(36) + Math.random().toString(36).slice(2, 6),
    rule_type: "domain_suffix",
    payload: "",
    outbound_tag: "proxy",
    enabled: true,
    description: "",
  };
  showModal.value = true;
}

async function submitRule() {
  if (!formRule.value.payload.trim()) return;

  const res = await addCustomRule({ ...formRule.value });
  if (res.success) {
    toast.success("规则已保存", `[${formRule.value.payload}] → ${formRule.value.outbound_tag}`);
    showModal.value = false;
    await fetchData();
  } else {
    toast.error("保存规则失败", res.error || "未知异常");
  }
}

async function toggleRuleEnabled(rule: CustomRuleItem) {
  rule.enabled = !rule.enabled;
  await addCustomRule(rule);
  toast.info(rule.enabled ? "规则已启用" : "规则已禁用", rule.payload);
}

async function handleOutboundChange(rule: CustomRuleItem, outbound: string) {
  rule.outbound_tag = outbound;
  await addCustomRule(rule);
  toast.success("分流策略已更新", `[${rule.payload}] → ${outbound}`);
}

async function handleDeleteRule(id: string) {
  const res = await deleteCustomRule(id);
  if (res.success) {
    rules.value = rules.value.filter((r) => r.id !== id);
    toast.success("规则已删除", "分流策略已同步热重载");
  }
}

onMounted(() => {
  fetchData();
});
</script>

<style scoped>
.custom-rules-container {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  overflow: hidden;
}

.rules-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  flex-shrink: 0;
}

.search-box {
  flex: 1;
  min-width: 220px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  transition: all 0.2s;
}

.search-box:focus-within {
  background: rgba(255, 255, 255, 0.06);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
}

.search-icon {
  font-size: 12px;
  opacity: 0.6;
}

.search-box input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: #fff;
  font-size: 12px;
}

.search-box input::placeholder {
  color: rgba(255, 255, 255, 0.35);
}

.btn-clear {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.4);
  cursor: pointer;
  font-size: 10px;
}

.type-capsules {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.capsule {
  padding: 4px 9px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.65);
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s;
}

.capsule:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}

.capsule.active {
  background: rgba(255, 255, 255, 0.14);
  border-color: rgba(255, 255, 255, 0.3);
  color: #fff;
}

.btn-add-rule {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  font-size: 12px;
  font-weight: 600;
  border-radius: 8px;
  border: none;
  cursor: pointer;
  background: color-mix(in srgb, var(--accent-cyan-vivid) 18%, transparent);
  color: var(--accent-cyan-vivid);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 35%, transparent);
  transition: all 0.2s;
}

.btn-add-rule:hover {
  background: var(--accent-cyan-vivid);
  color: #000;
}

.rules-body {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.rules-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-right: 3px;
}

.rule-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 12px;
  gap: 14px;
  transition: all 0.2s;
  flex-shrink: 0;
}

.rule-card:hover {
  background: rgba(255, 255, 255, 0.055);
  border-color: rgba(255, 255, 255, 0.15);
  transform: translateY(-1px);
}

.rule-card.disabled {
  opacity: 0.5;
}

.rule-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex: 1;
  min-width: 0;
}

.type-badge {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 5px;
  font-weight: 600;
  flex-shrink: 0;
}

.type-badge.domain_suffix { color: var(--accent-green); background: rgba(16, 185, 129, 0.12); border: 1px solid rgba(16, 185, 129, 0.25); }
.type-badge.domain { color: var(--accent-cyan-vivid); background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent); border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 25%, transparent); }
.type-badge.domain_keyword { color: #fbbf24; background: rgba(251, 191, 36, 0.12); border: 1px solid rgba(251, 191, 36, 0.25); }
.type-badge.ip_cidr { color: #f472b6; background: rgba(244, 114, 182, 0.12); border: 1px solid rgba(244, 114, 182, 0.25); }
.type-badge.domain_regex { color: #a78bfa; background: rgba(167, 139, 250, 0.12); border: 1px solid rgba(167, 139, 250, 0.25); }

.rule-details {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}

.payload-text {
  font-size: 13px;
  font-family: monospace;
  font-weight: 600;
  color: #fff;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.desc-text {
  font-size: 11px;
  color: rgba(255, 255, 255, 0.4);
}

.rule-arrow {
  display: flex;
  align-items: center;
  gap: 4px;
  color: rgba(255, 255, 255, 0.2);
  font-size: 11px;
  flex-shrink: 0;
}

.arrow-line {
  width: 24px;
  height: 1px;
  background: rgba(255, 255, 255, 0.15);
}

.rule-right {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.switch-wrap {
  display: flex;
  align-items: center;
  cursor: pointer;
}

.btn-delete {
  background: transparent;
  border: none;
  font-size: 14px;
  cursor: pointer;
  opacity: 0.5;
  transition: opacity 0.15s;
}

.btn-delete:hover {
  opacity: 1;
}

.state-box {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: rgba(255, 255, 255, 0.4);
  font-size: 13px;
  border-radius: 12px;
  border: 1px dashed rgba(255, 255, 255, 0.08);
}

.empty-icon {
  font-size: 32px;
  opacity: 0.4;
}

/* 模态弹窗 */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(10px);
  z-index: 10000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.modal-card {
  width: 480px;
  background: rgba(18, 20, 28, 0.98);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 14px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.7);
  overflow: hidden;
  animation: popIn 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

@keyframes popIn {
  from { opacity: 0; transform: scale(0.95); }
  to { opacity: 1; transform: scale(1); }
}

.modal-header {
  padding: 16px 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.modal-header h3 {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: #fff;
}

.btn-close {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.4);
  font-size: 14px;
  cursor: pointer;
}

.btn-close:hover {
  color: #fff;
}

.modal-body {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.7);
  font-weight: 500;
}

.type-selector-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.type-radio-btn {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s;
}

.type-radio-btn input {
  display: none;
}

.type-radio-btn:hover {
  background: rgba(255, 255, 255, 0.06);
}

.type-radio-btn.active {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent);
  border-color: var(--accent-cyan-vivid);
}

.radio-title {
  font-size: 12px;
  font-weight: 600;
  color: #fff;
}

.radio-sub {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.4);
}

.form-input {
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  padding: 8px 12px;
  color: #fff;
  font-size: 12.5px;
  outline: none;
}

.form-input:focus {
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 50%, transparent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent-cyan-vivid) 10%, transparent);
}

.form-input.mono {
  font-family: monospace;
}

.modal-footer {
  padding: 14px 20px;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.btn-modal-cancel {
  padding: 7px 14px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
  cursor: pointer;
}

.btn-modal-submit {
  padding: 7px 16px;
  background: var(--accent-cyan-vivid);
  color: #000;
  border: none;
  border-radius: 8px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.btn-modal-submit:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
</style>
