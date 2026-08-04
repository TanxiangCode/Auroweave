<script setup lang="ts">
/**
 * 节点工具栏
 * 作者: TanXiang
 *
 * 包含搜索框、排序切换、操作按钮（延迟测试/批量测速/刷新）
 */
import SvgIcon from "@/components/common/SvgIcon.vue";
import type { NodeSortConfig } from "@/types";

const props = defineProps<{
  /** 当前分组标签 */
  groupTag: string;
  /** 原始节点数量 */
  nodeCount: number;
  /** 筛选后节点数量 */
  filteredCount: number;
  /** 是否正在筛选 */
  isFiltering: boolean;
  /** 搜索文本 */
  searchText: string;
  /** 排序配置 */
  sortConfig: NodeSortConfig;
  /** 排序标签映射 */
  sortLabels: Record<string, string>;
}>();

const emit = defineEmits<{
  'update:searchText': [value: string];
  'cycle-sort': [];
  'toggle-sort-order': [];
  'run-latency': [];
  'show-batch-modal': [];
  refresh: [];
}>();
</script>

<template>
  <div class="nodes-toolbar">
    <!-- 左侧：标题 + 节点计数 -->
    <div class="toolbar-left">
      <h2>{{ groupTag }}</h2>
      <span class="nodes-count" v-if="nodeCount > 0">
        共 {{ nodeCount }} 个节点
        <span v-if="isFiltering" class="filter-count">（已筛选 {{ filteredCount }}）</span>
      </span>
    </div>

    <!-- 右侧：搜索 + 排序 + 操作 -->
    <div class="toolbar-right">
      <!-- 搜索框 -->
      <div class="search-box">
        <SvgIcon name="search" :size="12" class="search-icon" />
        <input
          :value="searchText"
          @input="emit('update:searchText', ($event.target as HTMLInputElement).value)"
          type="text"
          placeholder="搜索节点..."
          class="search-input"
        />
        <button v-if="searchText" class="search-clear" @click="emit('update:searchText', '')">×</button>
      </div>

      <!-- 排序按钮 -->
      <button class="btn-sort" @click="emit('cycle-sort')" :title="'排序: ' + sortLabels[sortConfig.key]">
        <SvgIcon name="sort" :size="12" class="icon-gap" />
        {{ sortLabels[sortConfig.key] }}
        <span class="sort-order" @click.stop="emit('toggle-sort-order')">{{ sortConfig.order === 'asc' ? '↑' : '↓' }}</span>
      </button>

      <!-- 操作按钮 -->
      <button class="btn-action" @click="emit('run-latency')" title="延迟测试">
        <SvgIcon name="bolt" :size="12" class="icon-gap" />
        测延迟
      </button>
      <button class="btn-action" @click="emit('show-batch-modal')" title="批量测速">
        <SvgIcon name="wifi" :size="12" class="icon-gap" />
        批量测速
      </button>
      <button class="btn-action" @click="emit('refresh')" title="刷新">
        <SvgIcon name="refresh" :size="12" class="icon-gap" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.nodes-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
  border-bottom: 1px solid var(--border-subtle);
  padding-bottom: var(--space-3);
  gap: var(--space-3);
}

.toolbar-left {
  display: flex;
  align-items: baseline;
  gap: var(--space-2);
  flex-shrink: 0;
}

.toolbar-left h2 {
  font-size: var(--text-md);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.nodes-count {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.filter-count {
  color: var(--accent-cyan);
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

/* 搜索框 */
.search-box {
  display: flex;
  align-items: center;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  padding: 0 var(--space-2);
  height: 30px;
  gap: var(--space-1);
  transition: border-color var(--duration-fast);
}

.search-box:focus-within {
  border-color: var(--accent-blue);
}

.search-icon {
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.search-input {
  background: transparent;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: var(--text-xs);
  width: 120px;
}

.search-input::placeholder {
  color: var(--text-tertiary);
}

.search-clear {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 16px;
  cursor: pointer;
  padding: 0 var(--space-1);
  line-height: 1;
}

.search-clear:hover {
  color: var(--text-primary);
}

/* 排序按钮 */
.btn-sort {
  display: flex;
  align-items: center;
  padding: 6px 10px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
  gap: var(--space-1);
}

.btn-sort:hover {
  background: var(--border-strong);
  border-color: var(--border-accent);
}

.sort-order {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  background: var(--border-subtle);
  border-radius: var(--radius-xs);
  font-size: 11px;
  cursor: pointer;
}

.sort-order:hover {
  background: var(--accent-blue-glow);
  color: var(--accent-blue);
}

/* 操作按钮 */
.btn-action {
  display: flex;
  align-items: center;
  padding: 6px 12px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-action:hover {
  background: var(--border-strong);
  border-color: var(--border-accent);
}

/* 图标间距工具类 */
.icon-gap {
  margin-right: 4px;
}
</style>
