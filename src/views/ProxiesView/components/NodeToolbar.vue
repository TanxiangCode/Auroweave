<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
/**
 * 节点工具栏 (双行版)
 * 作者: TanXiang
 *
 * 第一行：分组标题 + 节点数（左） / 搜索框（右）
 * 第二行：排序、视图切换、解锁筛选、定位当前节点、测延迟、批量测速、解锁检测、刷新
 */
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
    /** 当前分组内是否有选中节点（定位按钮可用性） */
    hasActiveNode?: boolean;
  }>(),
  {
    viewMode: "grid",
    isTestingLatency: false,
    isBatchSpeedTesting: false,
    isUnlockChecking: false,
    unlockFilter: "",
    hasActiveNode: false,
  }
);

const emit = defineEmits<{
  'update:searchText': [value: string];
  'cycle-sort': [];
  'toggle-sort-order': [];
  'run-latency': [];
  'show-batch-modal': [];
  'show-unlock-modal': [];
  'toggle-view-mode': [mode: "grid" | "list"];
  refresh: [];
  'update:unlockFilter': [value: string];
  'locate-active': [];
}>();
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
        @input="emit('update:searchText', ($event.target as HTMLInputElement).value)"
        type="text"
        placeholder="搜索节点名称/协议..."
        class="search-input"
      />
      <button v-if="searchText" class="search-clear" @click="emit('update:searchText', '')">×</button>
    </div>
  </div>

  <!-- 第二行：过滤与操作工具链 -->
  <div class="toolbar-row-actions">
    <!-- 排序切换按钮 -->
    <div class="sort-button-group">
      <button
        class="btn-sort"
        @click="emit('cycle-sort')"
        :title="`当前按 ${sortLabels[sortConfig.key]} 排序，点击切换排序字段`"
      >
        <SvgIcon name="sort" :size="12" class="icon-gap" />
        <span>{{ sortLabels[sortConfig.key] }}</span>
      </button>
      <button
        class="btn-sort-dir"
        @click.stop="emit('toggle-sort-order')"
        :title="sortConfig.order === 'asc' ? '升序 (点击切换为降序)' : '降序 (点击切换为升序)'"
      >
        {{ sortConfig.order === 'asc' ? '↑' : '↓' }}
      </button>
    </div>

    <!-- 视图模式切换 (Grid / List) -->
    <div class="view-mode-group">
      <button
        class="view-btn"
        :class="{ active: viewMode === 'grid' }"
        @click="emit('toggle-view-mode', 'grid')"
        title="网格卡片视图"
      >
        ▦
      </button>
      <button
        class="view-btn"
        :class="{ active: viewMode === 'list' }"
        @click="emit('toggle-view-mode', 'list')"
        title="紧凑列表视图"
      >
        <BaseIcon name="List" :size="14" />
      </button>
    </div>

    <!-- 解锁服务筛选片（按检测结果过滤节点列表） -->
    <div class="unlock-filter-group">
      <select
        class="unlock-filter-select"
        :value="unlockFilter"
        title="按 AI 服务解锁状态筛选节点"
        @change="emit('update:unlockFilter', ($event.target as HTMLSelectElement).value)"
      >
        <option value="">全部节点</option>
        <option value="gemini:yes">Gemini 可用</option>
        <option value="claude:yes">Claude 可用</option>
        <option value="chatgpt:yes">ChatGPT 可用</option>
        <option value="gemini:no">Gemini 封锁</option>
        <option value="claude:no">Claude 封锁</option>
      </select>
    </div>

    <!-- 定位当前节点 -->
    <button
      class="btn-action locate"
      :disabled="!hasActiveNode"
      @click="emit('locate-active')"
      :title="hasActiveNode ? '滚动到当前选中节点' : '当前分组暂无选中节点'"
    >
      <BaseIcon name="Crosshair" :size="12" class="icon-gap" />
      <span>定位</span>
    </button>

    <div class="divider-vertical"></div>

    <!-- 操作按钮组 -->
    <button
      class="btn-action ping"
      :class="{ loading: isTestingLatency }"
      :disabled="isTestingLatency"
      @click="emit('run-latency')"
      title="并发测试全部节点延迟"
    >
      <span v-if="isTestingLatency" class="spinner-ring"></span>
      <SvgIcon v-else name="bolt" :size="12" class="icon-gap" />
      <span>{{ isTestingLatency ? '测试中...' : '测延迟' }}</span>
    </button>

    <button
      class="btn-action speed"
      :class="{ loading: isBatchSpeedTesting }"
      :disabled="isBatchSpeedTesting"
      @click="emit('show-batch-modal')"
      title="开启批量吞吐量下载测速"
    >
      <span v-if="isBatchSpeedTesting" class="spinner-ring speed"></span>
      <SvgIcon v-else name="wifi" :size="12" class="icon-gap" />
      <span>{{ isBatchSpeedTesting ? '测速中...' : '批量测速' }}</span>
    </button>

    <button
      class="btn-action unlock"
      :class="{ loading: isUnlockChecking }"
      :disabled="isUnlockChecking"
      @click="emit('show-unlock-modal')"
      title="批量检测 AI 服务解锁状态（Gemini/Claude/ChatGPT）"
    >
      <span v-if="isUnlockChecking" class="spinner-ring unlock"></span>
      <BaseIcon v-else name="Sparkles" :size="12" class="icon-gap" />
      <span>{{ isUnlockChecking ? '检测中...' : '解锁检测' }}</span>
    </button>

    <button
      class="btn-action refresh"
      @click="emit('refresh')"
      title="刷新节点与策略组列表"
    >
      <SvgIcon name="refresh" :size="12" />
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
  flex-wrap: wrap;
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

/* 排序按钮组 */
.sort-button-group {
  display: inline-flex;
  align-items: center;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  height: 30px;
  overflow: hidden;
}

.btn-sort {
  display: flex;
  align-items: center;
  padding: 0 8px;
  background: transparent;
  border: none;
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background var(--duration-fast);
}

.btn-sort:hover {
  background: var(--border-subtle);
}

.btn-sort-dir {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 100%;
  background: var(--layer-3);
  border: none;
  border-left: 1px solid var(--border-subtle);
  color: var(--accent-cyan);
  font-weight: var(--weight-bold);
  font-size: 11px;
  cursor: pointer;
  transition: background var(--duration-fast);
}

.btn-sort-dir:hover {
  background: var(--border-strong);
}

/* 视图模式切换 */
.view-mode-group {
  display: inline-flex;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  height: 30px;
  overflow: hidden;
}

.view-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 100%;
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 13px;
  cursor: pointer;
  transition: all var(--duration-fast);
}

.view-btn:hover {
  color: var(--text-primary);
  background: var(--border-subtle);
}

.view-btn.active {
  background: var(--accent-cyan-glow);
  color: var(--accent-cyan);
  font-weight: var(--weight-bold);
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

/* 解锁服务筛选下拉 */
.unlock-filter-group {
  display: inline-flex;
  align-items: center;
  height: 30px;
}

.unlock-filter-select {
  height: 30px;
  padding: 0 6px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  outline: none;
  transition: all var(--duration-fast);
}

.unlock-filter-select:hover {
  border-color: var(--border-accent);
}

.unlock-filter-select:focus {
  border-color: var(--accent-cyan);
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
