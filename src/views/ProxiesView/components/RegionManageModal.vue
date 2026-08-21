<script setup lang="ts">
/**
 * 自定义区域管理弹窗
 * 作者: TanXiang
 *
 * 包含：规则列表、规则编辑/新增表单、内置区域快捷填充
 */
import { useProxyStore } from "@/stores/proxy.store";
import { useRegionRules } from "../hooks/useRegionRules";
import SvgIcon from "@/components/common/SvgIcon.vue";

defineProps<{
  visible: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

const proxyStore = useProxyStore();
const {
  editingRule,
  isNewRule,
  builtinRegions,
  addNewRule,
  editRule,
  applyBuiltinRegion,
  saveRule,
  deleteRule,
} = useRegionRules();
</script>

<template>
  <Teleport to="body">
    <div v-if="visible" class="modal-backdrop" @click.self="emit('close')">
      <div class="modal-card glass-effect region-modal">
        <h3>自定义区域管理</h3>

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
            </select>
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

          <!-- 内置区域快捷填充 -->
          <div v-if="isNewRule" class="builtin-regions">
            <span class="form-hint">快捷填充内置区域：</span>
            <div class="builtin-tags">
              <button
                v-for="region in builtinRegions"
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
                <span class="rule-type">{{ rule.match_type }}</span>
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
