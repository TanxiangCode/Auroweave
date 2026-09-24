<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
/**
 * 自定义区域管理弹窗
 * 作者: TanXiang
 *
 * 包含：规则列表、规则编辑/新增表单、内置区域快捷填充
 */
import { useProxyStore } from "@/stores/proxy.store";
import { useRegionRules } from "../hooks/useRegionRules";
import { builtinRegionGroups } from "../utils/builtin-regions";
import { ref, computed } from "vue";
import SvgIcon from "@/components/common/SvgIcon.vue";

defineProps<{
  visible: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

/** 匹配方式徽章中文标签 */
function matchTypeLabel(t: string): string {
  switch (t) {
    case "keyword": return "关键词";
    case "regex": return "正则";
    case "protocol": return "协议";
    case "unlock": return "解锁";
    default: return t;
  }
}

/** 快捷填充分洲：当前激活的洲（默认亚洲，避免 60+ 按钮平铺把弹窗撑高） */
const activeContinent = ref(builtinRegionGroups[0]?.continent ?? "");
const activeContinentRegions = computed(() => {
  return builtinRegionGroups.find((g) => g.continent === activeContinent.value)?.regions ?? [];
});

const proxyStore = useProxyStore();
const {
  editingRule,
  isNewRule,
  addNewRule,
  editRule,
  applyBuiltinRegion,
  saveRule,
  deleteRule,
} = useRegionRules();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop">
      <div class="modal-card glass-effect region-modal">
        <h3><BaseIcon name="Globe" :size="16" /> 自定义区域管理</h3>

        <!-- 编辑/新增表单 -->
        <div v-if="editingRule" class="rule-edit-form">
          <div class="form-row">
            <label>区域名称</label>
            <input v-model="editingRule.name" type="text" class="form-input" placeholder="如: 欧洲" />
          </div>
          <div class="form-row">
            <label>匹配方式</label>
            <select v-model="editingRule.match_type" class="form-input">
              <option value="keyword">关键词匹配</option>
              <option value="regex">正则表达式</option>
              <option value="protocol">协议类型</option>
              <option value="unlock">解锁检测结果</option>
            </select>
          </div>
          <div class="form-row">
            <label>分组类型</label>
            <select
              v-model="editingRule.group_type"
              class="form-input"
              @change="editingRule.group_type = ($event.target as HTMLSelectElement).value as any"
            >
              <option value="virtual">仅本地匹配（不进内核）</option>
              <option value="selector">Selector 手动选择</option>
              <option value="urltest">URLTest 自动优选</option>
              <option value="balance">Balance 独立自动优选</option>
            </select>
            <span v-if="editingRule.group_type && editingRule.group_type !== 'virtual'" class="form-hint">
              将生成真实策略组 custom-{{ editingRule.name || '…' }} 并重启内核，可在节点列表中直接切换
            </span>
          </div>
          <div v-if="editingRule.match_type === 'keyword'" class="form-row">
            <label>关键词列表</label>
            <input
              :value="editingRule.keywords.join(', ')"
              @input="editingRule.keywords = ($event.target as HTMLInputElement).value.split(',').map(s => s.trim()).filter(Boolean)"
              type="text"
              class="form-input"
              placeholder="用逗号分隔，如: EU, Europe, 欧洲"
            />
          </div>
          <div v-if="editingRule.match_type === 'regex'" class="form-row">
            <label>正则表达式</label>
            <input v-model="editingRule.pattern" type="text" class="form-input" placeholder="如: ^(EU|Europe)" />
          </div>
          <div v-if="editingRule.match_type === 'protocol'" class="form-row">
            <label>协议列表</label>
            <input
              :value="editingRule.protocols.join(', ')"
              @input="editingRule.protocols = ($event.target as HTMLInputElement).value.split(',').map(s => s.trim()).filter(Boolean)"
              type="text"
              class="form-input"
              placeholder="用逗号分隔，如: vmess, trojan"
            />
          </div>
          <div v-if="editingRule.match_type === 'unlock'" class="form-row unlock-match-row">
            <!-- 解锁匹配配置：服务 + 期望状态（保存时校验非空） -->
            <select
              :value="editingRule.unlock?.service ?? 'gemini'"
              @input="editingRule.unlock = { service: ($event.target as HTMLSelectElement).value as any, status: editingRule.unlock?.status ?? 'yes' }"
              class="form-input"
            >
              <option value="gemini">Gemini 可用性</option>
              <option value="claude">Claude 可用性</option>
              <option value="chatgpt">ChatGPT 可用性</option>
            </select>
            <select
              :value="editingRule.unlock?.status ?? 'yes'"
              @input="editingRule.unlock = { service: editingRule.unlock?.service ?? 'gemini', status: ($event.target as HTMLSelectElement).value as any }"
              class="form-input"
            >
              <option value="yes">可用</option>
              <option value="no">地区封锁</option>
              <option value="risky">风控疑似</option>
              <option value="failed">不可达</option>
            </select>
          </div>
          <div v-if="editingRule.match_type === 'unlock'" class="form-hint unlock-hint">
            按节点最近一次解锁检测结果匹配；从未检测的节点不会命中任何状态。
          </div>

          <!-- 内置区域快捷填充（按洲分组，避免 60+ 按钮平铺撑高弹窗） -->
          <div v-if="isNewRule" class="builtin-regions">
            <span class="form-hint">快捷填充内置区域：</span>
            <div class="builtin-tabs">
              <button
                v-for="g in builtinRegionGroups"
                :key="g.continent"
                class="builtin-tab"
                :class="{ active: activeContinent === g.continent }"
                @click="activeContinent = g.continent"
              >
                {{ g.continent }}
              </button>
            </div>
            <div class="builtin-tags">
              <button
                v-for="region in activeContinentRegions"
                :key="region.name"
                class="builtin-tag"
                @click="applyBuiltinRegion(region)"
              >
                {{ region.name }}
              </button>
            </div>
          </div>

          <div class="modal-actions">
            <button class="btn text" @click="editingRule = null">取消</button>
            <button class="btn primary" @click="saveRule">保存规则</button>
          </div>
        </div>

        <!-- 规则列表 -->
        <div v-else>
          <div v-if="proxyStore.customGroupRules.length === 0" class="empty-rules">
            暂无自定义区域规则，点击下方按钮添加
          </div>
          <div v-else class="rules-list">
            <div v-for="rule in proxyStore.customGroupRules" :key="rule.id" class="rule-item">
              <div class="rule-info">
                <span class="rule-name">{{ rule.name }}</span>
                <span class="rule-type">{{ matchTypeLabel(rule.match_type) }}</span>
                <span class="rule-enabled" :class="{ disabled: !rule.enabled }">
                  {{ rule.enabled ? '启用' : '禁用' }}
                </span>
              </div>
              <div class="rule-actions">
                <button class="btn-icon" @click="editRule(rule)" title="编辑">
                  <SvgIcon name="edit" :size="12" />
                </button>
                <button class="btn-icon btn-delete-icon" @click="deleteRule(rule.id)" title="删除">
                  <SvgIcon name="trash" :size="12" />
                </button>
              </div>
            </div>
          </div>
          <div class="modal-actions">
            <button class="btn text" @click="emit('close')">关闭</button>
            <button class="btn primary" @click="addNewRule">
              <SvgIcon name="plus" :size="12" class="icon-gap" />
              新增区域
            </button>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.region-modal {
  width: 460px;
  max-height: 80vh;
  overflow-y: auto;
}

h3 {
  font-size: var(--text-md);
  font-weight: var(--weight-bold);
}

.empty-rules {
  text-align: center;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  padding: var(--space-5) 0;
}

.rules-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  /* 规则多时限高滚动，防撑高弹窗 */
  max-height: 45vh;
  overflow-y: auto;
}

.rule-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-3);
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
}

.rule-info {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1;
  min-width: 0;
}

.rule-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.rule-type {
  font-size: var(--text-xs);
  padding: 1px var(--space-2);
  background: var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.rule-enabled {
  font-size: var(--text-xs);
  color: var(--accent-green);
}

.rule-enabled.disabled {
  color: var(--text-tertiary);
}

.rule-actions {
  display: flex;
  gap: var(--space-1);
}

.btn-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-icon:hover {
  color: var(--text-primary);
  border-color: var(--border-normal);
}

.btn-delete-icon:hover {
  color: var(--accent-red);
  border-color: var(--accent-red);
}

.builtin-regions {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

/* 洲切换 tab：一行紧凑胶囊，控制单屏按钮数量 */
.builtin-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
}

.builtin-tab {
  padding: 3px 10px;
  background: transparent;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full);
  color: var(--text-tertiary);
  font-size: 11px;
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.builtin-tab.active {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
  font-weight: var(--weight-semibold);
}

.rule-edit-form {
  /* 修复表单拥挤：行间拉开 14px，label 与控件间 6px */
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.rule-edit-form .form-row {
  gap: 6px;
}

/* 解锁匹配双下拉行：覆盖全局 form-row 的 column 方向，两个下拉并排 */
.unlock-match-row {
  display: flex;
  flex-direction: row;
  gap: var(--space-2);
}

.unlock-match-row .form-input {
  flex: 1;
}

.unlock-hint {
  font-size: 11px;
  color: var(--text-tertiary);
  line-height: 1.5;
  margin-top: calc(-1 * var(--space-1));
}

.builtin-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.builtin-tag {
  padding: var(--space-1) var(--space-3);
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-full);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.builtin-tag:hover {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
}

.icon-gap {
  margin-right: 4px;
}
</style>
