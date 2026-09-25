<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
/**
 * 节点工具栏 (双行版)
 * 作者: TanXiang
 *
 * 第一行：分组标题 + 节点数（左） / 搜索框（右）
 * 第二行：排序/视图/筛选收纳为 3 个控件，主测速按钮置右，定位与刷新改图标按钮
 */
import { ref } from "vue";
import SvgIcon from "@/components/common/SvgIcon.vue";
import type { NodeSortConfig } from "@/types";

withDefaults(
  defineProps<{
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
    /** 视图模式 */
    viewMode?: "grid" | "list";
    /** 是否正在测延迟 */
    isTestingLatency?: boolean;
    /** 是否正在批量吞吐量测速 */
    isBatchSpeedTesting?: boolean;
    /** 是否正在批量解锁检测 */
    isUnlockChecking?: boolean;
    /** 服务筛选状态（空串=未筛选） */
    unlockFilter?: string;
    /** 是否隐藏已经测出超时的节点 */
    hideTimedOut?: boolean;
    /** 当前分组内是否有选中节点（定位按钮可用性） */
    hasActiveNode?: boolean;
  }>(),
  {
    viewMode: "grid",
    isTestingLatency: false,
    isBatchSpeedTesting: false,
    isUnlockChecking: false,
    unlockFilter: "",
    hideTimedOut: false,
    hasActiveNode: false,
  }
);

const emit = defineEmits<{
  'update:searchText': [value: string];
  'update-sort': [value: NodeSortConfig];
  'run-latency': [];
  'show-batch-modal': [];
  'show-unlock-modal': [];
  'toggle-view-mode': [mode: "grid" | "list"];
  refresh: [];
  'update:unlockFilter': [value: string];
  'update:hideTimedOut': [value: boolean];
  'locate-active': [];
}>();

const filterMenu = ref<HTMLDetailsElement | null>(null);

function onSortChange(event: Event) {
  const [key, order] = (event.target as HTMLSelectElement).value.split(":");
  emit("update-sort", {
    key: key as NodeSortConfig["key"],
    order: order as NodeSortConfig["order"],
  });
}

function closeFilterMenu() {
  filterMenu.value?.removeAttribute("open");
}
</script>

<template>
  <div class="nodes-toolbar">
  <!-- 第一行：分组标题 + 统计（左） / 搜索框（右） -->
  <div class="toolbar-row-primary">
    <div class="toolbar-left">
      <div class="group-title-row">
        <h2 class="group-title">{{ groupTag }}</h2>
        <span class="nodes-count-pill">
          {{ filteredCount }}
          <span v-if="isFiltering" class="filter-total">/ {{ nodeCount }}</span>
          个节点
        </span>
      </div>
    </div>

    <div class="search-box">
      <SvgIcon name="search" :size="12" class="search-icon" />
      <input
        :value="searchText"
        class="search-input"
        type="text"
        placeholder="搜索节点名称/协议..."
        @input="emit('update:searchText', ($event.target as HTMLInputElement).value)"
      />
      <button v-if="searchText" class="search-clear" @click="emit('update:searchText', '')">×</button>
    </div>
  </div>

  <!-- 第二行：次要工具收纳 + 主测试操作 -->
  <div class="toolbar-row-actions">
    <!-- 排序字段与方向合并为单个下拉 -->
    <div class="toolbar-select-wrap" title="节点排序">
      <SvgIcon name="sort" :size="12" />
      <select
        class="toolbar-select"
        :value="`${sortConfig.key}:${sortConfig.order}`"
        @change="onSortChange"
      >
        <option
          v-for="(label, key) in sortLabels"
          :key="`${key}:asc`"
          :value="`${key}:asc`"
        >{{ label }} ↑</option>
        <option
          v-for="(label, key) in sortLabels"
          :key="`${key}:desc`"
          :value="`${key}:desc`"
        >{{ label }} ↓</option>
      </select>
    </div>

    <!-- 网格/列表合并为一个切换按钮 -->
    <button
      class="toolbar-icon-control"
      :title="viewMode === 'grid' ? '切换为列表视图' : '切换为网格视图'"
      @click="emit('toggle-view-mode', viewMode === 'grid' ? 'list' : 'grid')"
    >
      <BaseIcon :name="viewMode === 'grid' ? 'List' : 'LayoutGrid'" :size="13" />
    </button>

    <!-- 解锁状态与超时筛选收纳到同一菜单 -->
    <details ref="filterMenu" class="filter-menu">
      <summary
        class="toolbar-filter-trigger"
        :class="{ active: unlockFilter || hideTimedOut }"
        title="节点筛选"
      >
        <BaseIcon name="SlidersHorizontal" :size="12" />
        <span>筛选</span>
        <span v-if="unlockFilter || hideTimedOut" class="filter-count">1</span>
      </summary>
      <div class="filter-popover" @click.stop>
        <label class="filter-checkbox-row">
          <input
            type="checkbox"
            :checked="hideTimedOut"
            @change="emit('update:hideTimedOut', ($event.target as HTMLInputElement).checked)"
          />
          <span>隐藏超时节点</span>
        </label>
        <div class="filter-separator"></div>
        <label class="filter-select-label" for="unlock-status-filter">解锁状态</label>
        <select
          id="unlock-status-filter"
          class="filter-select"
          :value="unlockFilter"
          @change="emit('update:unlockFilter', ($event.target as HTMLSelectElement).value); closeFilterMenu()"
        >
          <option value="">全部节点</option>
          <option value="gemini:yes">Gemini 可用</option>
          <option value="claude:yes">Claude 可用</option>
          <option value="chatgpt:yes">ChatGPT 可用</option>
          <option value="gemini:no">Gemini 封锁</option>
          <option value="claude:no">Claude 封锁</option>
        </select>
      </div>
    </details>

    <!-- 定位当前节点（次要工具改为图标） -->
    <button
      class="toolbar-icon-control locate"
      :disabled="!hasActiveNode"
      :title="hasActiveNode ? '滚动到当前选中节点' : '当前分组暂无选中节点'"
      @click="emit('locate-active')"
    >
      <BaseIcon name="Crosshair" :size="13" />
    </button>

    <div class="divider-vertical"></div>

    <!-- 三个主测试操作保留文字，目标为当前筛选后的可见节点 -->
    <button
      class="btn-action ping"
      :class="{ loading: isTestingLatency }"
      :disabled="isTestingLatency"
      title="测试当前筛选后的可见节点延迟"
      @click="emit('run-latency')"
    >
      <span v-if="isTestingLatency" class="spinner-ring"></span>
      <SvgIcon v-else name="bolt" :size="12" class="icon-gap" />
      <span>{{ isTestingLatency ? '测试中...' : '延迟' }}</span>
    </button>

    <button
      class="btn-action speed"
      :class="{ loading: isBatchSpeedTesting }"
      :disabled="isBatchSpeedTesting"
      title="测试当前筛选后的可见节点速度"
      @click="emit('show-batch-modal')"
    >
      <span v-if="isBatchSpeedTesting" class="spinner-ring speed"></span>
      <SvgIcon v-else name="wifi" :size="12" class="icon-gap" />
      <span>{{ isBatchSpeedTesting ? '测速中...' : '测速' }}</span>
    </button>

    <button
      class="btn-action unlock"
      :class="{ loading: isUnlockChecking }"
      :disabled="isUnlockChecking"
      title="检测当前筛选后的可见节点解锁状态"
      @click="emit('show-unlock-modal')"
    >
      <span v-if="isUnlockChecking" class="spinner-ring unlock"></span>
      <BaseIcon v-else name="Sparkles" :size="12" class="icon-gap" />
      <span>{{ isUnlockChecking ? '检测中...' : '解锁' }}</span>
    </button>

    <div class="toolbar-spacer"></div>

    <!-- 刷新固定在最右侧 -->
    <button
      class="toolbar-icon-control refresh"
      title="刷新节点与策略组列表"
      @click="emit('refresh')"
    >
      <SvgIcon name="refresh" :size="13" />
    </button>
  </div>
  </div>
</template>

<style scoped>
/* 双行容器 */
.nodes-toolbar {
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  gap: 0;
}

/* 第一行：标题/统计 左 — 搜索 右（纯间距分隔，不画分隔线） */
.toolbar-row-primary {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
  gap: var(--space-3);
  flex-wrap: wrap;
  padding-bottom: var(--space-3);
}

/* 第二行：操作工具链 */
.toolbar-row-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: nowrap;
  min-width: 0;
  padding-top: var(--space-3);
}

.toolbar-left {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.group-title-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.group-title {
  font-size: var(--text-md);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
  letter-spacing: -0.2px;
}

.nodes-count-pill {
  font-size: 11px;
  padding: 2px 8px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full);
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

.filter-total {
  color: var(--text-tertiary);
}

/* 搜索框（第一行右侧） */
.search-box {
  display: flex;
  align-items: center;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  padding: 0 8px;
  height: 30px;
  gap: 6px;
  transition: all var(--duration-fast);
  min-width: 200px;
  max-width: 320px;
  flex: 0 1 auto;
}

.search-box:focus-within {
  border-color: var(--accent-cyan);
  box-shadow: 0 0 6px var(--accent-cyan-glow);
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
  width: 100%;
}

.search-input::placeholder {
  color: var(--text-tertiary);
}

.search-clear {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 14px;
  cursor: pointer;
  padding: 0 2px;
  line-height: 1;
}

.search-clear:hover {
  color: var(--text-primary);
}

/* 排序、视图与筛选：三个紧凑控件 */
.toolbar-select-wrap {
  display: inline-flex;
  align-items: center;
  height: 30px;
  padding-left: 8px;
  color: var(--text-tertiary);
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
}

.toolbar-select {
  height: 28px;
  max-width: 126px;
  padding: 0 6px 0 4px;
  color: var(--text-primary);
  background: transparent;
  border: 0;
  outline: none;
  font-size: var(--text-xs);
  cursor: pointer;
}

.toolbar-select option,
.filter-select option {
  color: var(--text-primary);
  background: var(--layer-1);
}

.toolbar-icon-control {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  padding: 0;
  color: var(--text-secondary);
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.toolbar-icon-control:hover:not(:disabled) {
  color: var(--text-primary);
  background: var(--border-strong);
  border-color: var(--border-accent);
}

.toolbar-icon-control.locate:hover:not(:disabled) {
  color: var(--accent-blue);
  border-color: var(--accent-blue);
}

.toolbar-icon-control:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.filter-menu {
  position: relative;
}

.filter-menu summary::-webkit-details-marker {
  display: none;
}

.toolbar-filter-trigger {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 30px;
  padding: 0 8px;
  color: var(--text-primary);
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  list-style: none;
  cursor: pointer;
  user-select: none;
}

.toolbar-filter-trigger:hover,
.toolbar-filter-trigger.active,
.filter-menu[open] .toolbar-filter-trigger {
  color: var(--accent-cyan);
  border-color: var(--accent-cyan);
  background: var(--accent-cyan-glow);
}

.filter-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  color: var(--text-on-accent);
  background: var(--accent-cyan);
  border-radius: var(--radius-full);
  font-size: 9px;
}

.filter-popover {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 60;
  width: 210px;
  padding: 10px;
  color: var(--text-primary);
  background: var(--bg-surface-elevated, var(--layer-1));
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.28);
}

.filter-checkbox-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 28px;
  font-size: var(--text-xs);
  cursor: pointer;
}

.filter-checkbox-row input {
  accent-color: var(--accent-cyan);
}

.filter-separator {
  height: 1px;
  margin: 8px 0;
  background: var(--border-subtle);
}

.filter-select-label {
  display: block;
  margin-bottom: 6px;
  color: var(--text-tertiary);
  font-size: 10px;
}

.filter-select {
  width: 100%;
  height: 30px;
  padding: 0 7px;
  color: var(--text-primary);
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  outline: none;
  cursor: pointer;
}

.toolbar-spacer {
  flex: 1 1 auto;
  min-width: 4px;
}

.divider-vertical {
  width: 1px;
  height: 20px;
  background: var(--border-normal);
  margin: 0 2px;
}

/* 操作按钮 */
.btn-action {
  display: flex;
  align-items: center;
  padding: 0 10px;
  height: 30px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-action:hover:not(:disabled) {
  background: var(--border-strong);
  border-color: var(--border-accent);
}

.btn-action.locate:hover:not(:disabled) {
  color: var(--accent-blue);
  border-color: var(--accent-blue);
}

.btn-action.ping:hover:not(:disabled) {
  color: var(--accent-orange);
  border-color: var(--accent-orange);
}

.btn-action.ping.loading {
  background: var(--layer-2);
  border-color: var(--accent-orange);
  color: var(--accent-orange);
  opacity: 0.9;
  cursor: wait;
}

.btn-action.speed:hover:not(:disabled) {
  color: var(--accent-cyan);
  border-color: var(--accent-cyan);
}

.btn-action.speed.loading {
  background: var(--layer-2);
  border-color: var(--accent-cyan);
  color: var(--accent-cyan);
  opacity: 0.9;
  cursor: wait;
}

.btn-action.unlock:hover:not(:disabled) {
  color: var(--accent-green);
  border-color: var(--accent-green);
}

.btn-action.unlock.loading {
  background: var(--layer-2);
  border-color: var(--accent-green);
  color: var(--accent-green);
  opacity: 0.9;
  cursor: wait;
}

.btn-action:disabled:not(.loading) {
  opacity: 0.6;
  cursor: not-allowed;
}

.spinner-ring {
  width: 12px;
  height: 12px;
  border: 2px solid rgba(245, 158, 11, 0.25);
  border-top-color: var(--accent-orange);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin-right: 6px;
  flex-shrink: 0;
}

.spinner-ring.speed {
  border-color: rgba(34, 211, 238, 0.25);
  border-top-color: var(--accent-cyan);
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.icon-gap {
  margin-right: 4px;
}
</style>
