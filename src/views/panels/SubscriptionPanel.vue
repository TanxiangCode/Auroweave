<template>
  <div class="panel-container">
    <div class="panel-header-row">
      <h2>
        <BaseIcon name="Rss" :size="20" class="panel-header-icon" />
        订阅管理
      </h2>
      <button class="btn-goto-full" @click="$router.push('/subscriptions')">
        <BaseIcon name="Rss" :size="13" />
        <span>打开全屏订阅中心</span>
        <BaseIcon name="ChevronRight" :size="13" />
      </button>
    </div>

    <!-- 订阅相关设置（唯一写入口） -->
    <div class="setting-card glass-effect">
      <div class="card-header">
        <span class="card-icon">
          <BaseIcon name="Sliders" :size="20" />
        </span>
        <div class="card-title-group">
          <h3>订阅行为</h3>
          <p>导入与聚合的基础开关，编辑/删除/切换等全部操作收敛到订阅中心</p>
        </div>
      </div>
      <div class="card-body">
        <div class="setting-item">
          <div class="item-label">
            <span>自动整理地区分组</span>
            <span class="sub-label">导入订阅时自动识别 HK/JP/US 等地区并生成 urltest 延迟优选组</span>
          </div>
          <input type="checkbox" v-model="settingsStore.settings.auto_group_on_import" class="switch" @change="save" />
        </div>
      </div>
    </div>

    <!-- 订阅概览（只读；操作去订阅中心） -->
    <div class="overview-card glass-effect">
      <div class="overview-head">
        <h3>
          <BaseIcon name="List" :size="16" />
          订阅概览
        </h3>
        <span class="overview-stat">
          <strong>{{ activeCount }}</strong> 个聚合中 / 共 {{ subscriptions.length }} 个
        </span>
      </div>

      <EmptyState
        v-if="subscriptions.length === 0"
        icon="Rss"
        title="尚未导入任何订阅"
        description="订阅是节点与分流配置的来源，导入后才能接管流量"
        cta-text="前往订阅中心导入"
        cta-icon="Plus"
        @cta="$router.push('/subscriptions')"
      />

      <div v-else class="subscription-list">
        <div
          v-for="sub in subscriptions"
          :key="sub.id"
          class="subscription-item"
          :class="{ active: sub.is_active }"
        >
          <div class="sub-info">
            <div class="sub-header">
              <span class="sub-name">{{ sub.name }}</span>
              <span v-if="sub.is_active" class="active-badge">聚合中</span>
            </div>
            <div class="sub-meta">
              <span class="sub-format">{{ (sub.format || 'unknown').toUpperCase() }}</span>
              <span class="sub-nodes">{{ sub.node_count || 0 }} 节点</span>
              <span v-if="sub.last_updated" class="sub-time">
                更新于 {{ formatTime(sub.last_updated) }}
              </span>
            </div>
          </div>
          <button
            class="btn-action btn-goto-item"
            @click="$router.push('/subscriptions')"
            :title="`${sub.name} · 前往订阅中心操作（切换聚合/刷新/编辑/删除）`"
          >
            管理
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 设置页订阅面板（M3-4 双入口收敛后）
 * 作者: TanXiang
 *
 * 历史上面板与 SubscriptionsView 是功能全量重复的双入口，且交互语义已漂移
 * （面板为单订阅切换旧语义，订阅中心为多订阅聚合 toggle 新语义）。本轮收敛：
 * - 面板仅保留：订阅设置开关 + 只读订阅概览 + 跳转订阅中心
 * - 切换聚合/刷新/编辑/删除/导入/批量更新全部收敛到 SubscriptionsView，
 *   行为语义单一真相源，无双入口分歧
 */
import { computed, onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useSettingsStore } from "@/stores/settings.store";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { useToast } from "@/composables/useToast";
import BaseIcon from "@/components/common/BaseIcon.vue";
import EmptyState from "@/components/common/EmptyState.vue";

const settingsStore = useSettingsStore();
const subStore = useSubscriptionStore();
const toast = useToast();

const { subscriptions } = storeToRefs(subStore);

/** 聚合中的订阅数量（多订阅语义，与订阅中心一致） */
const activeCount = computed(() => subscriptions.value.filter((s) => s.is_active).length);

onMounted(() => {
  subStore.fetchAll();
});

/** 保存订阅设置开关（patch 式：仅提交本面板字段） */
async function save() {
  await settingsStore.updateSettings({
    auto_group_on_import: settingsStore.settings.auto_group_on_import,
  });
  toast.success("订阅设置已保存");
}

function formatTime(ts?: number): string {
  if (!ts) return "";
  const d = new Date(ts);
  return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
}
</script>

<style scoped>
/* switch 统一走 App.vue 全局胶囊开关；setting-card 三线视觉走全局体系 */

.panel-container {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.panel-header-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.btn-goto-full {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--accent-cyan-vivid) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  color: var(--accent-cyan-vivid);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.btn-goto-full:hover {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 20%, transparent);
  color: #fff;
  transform: translateX(2px);
}

.overview-card {
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.overview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.overview-head h3 {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-base);
  font-weight: var(--weight-semibold);
}

.overview-stat {
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.overview-stat strong {
  color: var(--accent-cyan-vivid);
  font-weight: var(--weight-bold);
}

.subscription-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.subscription-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-3) var(--space-4);
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  gap: var(--space-3);
}

.subscription-item.active {
  background: var(--accent-cyan-glow);
  border-color: var(--border-accent);
}

.sub-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.sub-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.sub-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.active-badge {
  font-size: var(--text-xs);
  padding: var(--space-1) var(--space-2);
  background: var(--accent-cyan-vivid);
  color: var(--layer-0);
  border-radius: var(--radius-full);
  font-weight: var(--weight-bold);
  flex-shrink: 0;
}

.sub-meta {
  display: flex;
  gap: var(--space-3);
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.sub-format { font-family: var(--font-mono); }
.sub-nodes { font-weight: var(--weight-medium); }

.btn-action {
  padding: var(--space-1) var(--space-3);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  cursor: pointer;
  transition: all var(--duration-fast);
  flex-shrink: 0;
}

.btn-goto-item:hover {
  color: var(--accent-cyan-vivid);
  border-color: var(--border-accent);
}
</style>
