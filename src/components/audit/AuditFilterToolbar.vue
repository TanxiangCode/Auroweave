<template>
  <div class="audit-filter-toolbar">
    <!-- 第一行：视图模式切换 + 事件统计 + 悬停暂停开关（所有视图常驻） -->
    <div class="toolbar-primary-row">
      <!-- 左侧：事件统计与暂停提示 -->
      <div class="sub-left">
        <span class="match-info">共匹配 <strong class="highlight-num">{{ matchCount }}</strong> 条语义化安全事件</span>
        <span class="pause-state-pill" v-if="isPaused"><BaseIcon name="Pause" :size="10" /> 实时流已处于暂停状态</span>
      </div>

      <!-- 右侧：悬停暂停开关 + 视图切换 Tabs -->
      <div class="sub-right">
        <!-- 悬停自动暂停开关（仅语义流/明细表受益于悬停暂停） -->
        <label
          v-if="viewMode === 'semantic' || viewMode === 'table'"
          class="hover-pause-toggle"
          title="鼠标移入列表时是否自动暂停实时流以方便查看"
        >
          <input
            type="checkbox"
            :checked="autoPauseOnHover"
            @change="$emit('update:autoPauseOnHover', ($event.target as HTMLInputElement).checked)"
          />
          <span>悬停暂停</span>
        </label>

        <!-- 视图模式切换 -->
        <div class="view-mode-tabs">
          <button
            class="view-tab-btn"
            :class="{ active: viewMode === 'semantic' }"
            @click="$emit('update:viewMode', 'semantic')"
            title="智能语义化叙事卡片流"
          >
             语义流
          </button>
          <button
            class="view-tab-btn"
            :class="{ active: viewMode === 'table' }"
            @click="$emit('update:viewMode', 'table')"
            title="专业高密度拓扑连接明细表"
          >
             明细表
          </button>
          <button
            class="view-tab-btn"
            :class="{ active: viewMode === 'raw' }"
            @click="$emit('update:viewMode', 'raw')"
            title="底层内核原始终端日志"
          >
             内核日志
          </button>
          <button
            class="view-tab-btn"
            :class="{ active: viewMode === 'connectivity' }"
            @click="$emit('update:viewMode', 'connectivity')"
            title="连通性/出口/泄漏检测"
          >
             连通检测
          </button>
        </div>
      </div>
    </div>

    <!-- 第二行：搜索与多维状态/协议过滤 + 排序方式
         仅语义流/明细表参与过滤与排序；内核日志、连通检测视图整行隐藏 -->
    <div
      class="toolbar-sub-row"
      v-if="viewMode === 'semantic' || viewMode === 'table'"
    >
      <!-- 搜索框 -->
      <div class="search-box">
        <span class="search-icon"><BaseIcon name="Search" :size="14" /></span>
        <input
          :value="searchQuery"
          @input="$emit('update:searchQuery', ($event.target as HTMLInputElement).value)"
          type="text"
          placeholder="搜索进程名、目标域名、IP 地址、分流规则或节点..."
        />
        <button
          v-if="searchQuery"
          class="btn-clear-search"
          @click="$emit('update:searchQuery', '')"
        >
          <BaseIcon name="X" :size="12" />
        </button>
      </div>

      <!-- 排序方式选择器：全自绘下拉（原生 select 在 WebView 内点击区异常，
           嵌套按钮点不中；参照 OutboundSelector 的 Teleport 弹层模式重写） -->
      <div class="sort-picker" ref="sortPickerRef">
        <button
          class="sort-trigger"
          title="选择记录排序方式"
          @click="toggleSortMenu"
        >
          <span class="sort-trigger-label">{{ SORT_OPTIONS[sortMode].label }}</span>
          <BaseIcon name="ChevronDown" :size="12" class="sort-trigger-arrow" :class="{ rotated: sortMenuOpen }" />
        </button>
      </div>
      <button
        class="sort-order-btn"
        :title="sortAscending ? '当前升序，点击切换为降序' : '当前降序，点击切换为升序'"
        @click="$emit('update:sortAscending', !sortAscending)"
      >
        <BaseIcon :name="sortAscending ? 'ChevronUp' : 'ChevronDown'" :size="13" />
      </button>

      <!-- 活跃/历史显示开关 -->
      <label
        class="show-history-toggle"
        title="控制已断开的历史会话是否在列表中显示"
      >
        <input
          type="checkbox"
          :checked="showHistory"
          @change="$emit('update:showHistory', ($event.target as HTMLInputElement).checked)"
        />
        <span>{{ showHistory ? '含历史会话' : '仅活跃连接' }}</span>
      </label>

      <!-- 状态筛选胶囊 -->
      <div class="filter-capsules">
        <button
          class="capsule"
          :class="{ active: statusFilter === 'all' }"
          @click="$emit('update:statusFilter', 'all')"
        >
          <BaseIcon name="Layers" :size="13" /> 全部 ({{ totalCount }})
        </button>
        <button
          class="capsule proxied"
          :class="{ active: statusFilter === 'proxied' }"
          @click="$emit('update:statusFilter', 'proxied')"
        >
           加密代理
        </button>
        <button
          class="capsule direct"
          :class="{ active: statusFilter === 'direct' }"
          @click="$emit('update:statusFilter', 'direct')"
        >
           大陆直连
        </button>
        <button
          class="capsule blocked"
          :class="{ active: statusFilter === 'blocked' }"
          @click="$emit('update:statusFilter', 'blocked')"
        >
           安全拦截
        </button>
      </div>

      <!-- 协议筛选 -->
      <div class="proto-selector">
        <button
          class="proto-btn"
          :class="{ active: protocolFilter === 'all' }"
          @click="$emit('update:protocolFilter', 'all')"
        >
          ALL
        </button>
        <button
          class="proto-btn"
          :class="{ active: protocolFilter === 'tcp' }"
          @click="$emit('update:protocolFilter', 'tcp')"
        >
          TCP
        </button>
        <button
          class="proto-btn"
          :class="{ active: protocolFilter === 'udp' }"
          @click="$emit('update:protocolFilter', 'udp')"
        >
          UDP
        </button>
      </div>
    </div>

    <!-- 排序方式下拉弹层（Teleport 到 body，fixed 定位对齐触发按钮） -->
    <Teleport to="body">
      <Transition name="sortmenu">
        <div v-if="sortMenuOpen" ref="sortMenuRef" class="sort-menu glass-effect" :style="sortMenuStyle">
          <button
            v-for="opt in SORT_OPTION_LIST"
            :key="opt.value"
            class="sort-menu-item"
            :class="{ active: sortMode === opt.value }"
            @click="selectSortMode(opt.value)"
          >
            <span>{{ opt.label }}</span>
            <BaseIcon v-if="sortMode === opt.value" name="Check" :size="12" class="sort-menu-check" />
          </button>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
import { ref, onMounted, onUnmounted, nextTick } from "vue";

export type AuditSortMode = "default" | "time-desc" | "time-asc" | "traffic-desc" | "domain-asc";

const SORT_OPTIONS: Record<AuditSortMode, { label: string }> = {
  default: { label: "默认顺序" },
  "time-desc": { label: "最新连接在前" },
  "time-asc": { label: "最早连接在前" },
  "traffic-desc": { label: "流量最大在前" },
  "domain-asc": { label: "域名 A→Z" },
};
const SORT_OPTION_LIST = Object.entries(SORT_OPTIONS).map(([value, { label }]) => ({
  value: value as AuditSortMode,
  label,
}));

defineProps<{
  searchQuery: string;
  statusFilter: "all" | "proxied" | "direct" | "blocked";
  protocolFilter: "all" | "tcp" | "udp";
  viewMode: "semantic" | "table" | "raw" | "connectivity";
  autoPauseOnHover: boolean;
  sortMode: AuditSortMode;
  sortAscending: boolean;
  showHistory: boolean;
  totalCount: number;
  matchCount: number;
  isPaused: boolean;
}>();

const emit = defineEmits<{
  (e: "update:searchQuery", val: string): void;
  (e: "update:statusFilter", val: "all" | "proxied" | "direct" | "blocked"): void;
  (e: "update:protocolFilter", val: "all" | "tcp" | "udp"): void;
  (e: "update:viewMode", val: "semantic" | "table" | "raw" | "connectivity"): void;
  (e: "update:autoPauseOnHover", val: boolean): void;
  (e: "update:sortMode", val: AuditSortMode): void;
  (e: "update:sortAscending", val: boolean): void;
  (e: "update:showHistory", val: boolean): void;
}>();

// ---- 排序下拉弹层状态（Teleport + fixed 定位，参照 OutboundSelector 模式） ----
const sortMenuOpen = ref(false);
const sortPickerRef = ref<HTMLDivElement | null>(null);
const sortMenuRef = ref<HTMLDivElement | null>(null);
const sortMenuStyle = ref<Record<string, string>>({});

async function toggleSortMenu() {
  if (sortMenuOpen.value) {
    sortMenuOpen.value = false;
    return;
  }
  sortMenuOpen.value = true;
  await nextTick();
  updateSortMenuPosition();
}

function updateSortMenuPosition() {
  const el = sortPickerRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  const MENU_WIDTH = 150;
  const MENU_MAX_HEIGHT = 160;
  const openDownward = rect.bottom + 4 + MENU_MAX_HEIGHT <= window.innerHeight;
  sortMenuStyle.value = {
    position: "fixed",
    top: openDownward ? `${rect.bottom + 4}px` : `${Math.max(8, rect.top - 4 - MENU_MAX_HEIGHT)}px`,
    left: `${Math.max(8, Math.min(rect.left, window.innerWidth - MENU_WIDTH - 8))}px`,
    width: `${MENU_WIDTH}px`,
    zIndex: "9999",
  };
}

function selectSortMode(val: AuditSortMode) {
  emit("update:sortMode", val);
  sortMenuOpen.value = false;
}

function handleSortMenuOutside(e: MouseEvent) {
  const target = e.target as Node;
  if (
    sortMenuOpen.value &&
    !sortPickerRef.value?.contains(target) &&
    !sortMenuRef.value?.contains(target)
  ) {
    sortMenuOpen.value = false;
  }
}

function handleSortMenuReposition() {
  if (sortMenuOpen.value) updateSortMenuPosition();
}

onMounted(() => {
  document.addEventListener("click", handleSortMenuOutside);
  window.addEventListener("scroll", handleSortMenuReposition, true);
  window.addEventListener("resize", handleSortMenuReposition);
});

onUnmounted(() => {
  document.removeEventListener("click", handleSortMenuOutside);
  window.removeEventListener("scroll", handleSortMenuReposition, true);
  window.removeEventListener("resize", handleSortMenuReposition);
});
</script>

<style scoped>
.audit-filter-toolbar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

/* 第一行：视图切换行（所有视图常驻） */
.toolbar-primary-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 2px 4px;
}

.sub-left {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 11.5px;
  color: rgba(255, 255, 255, 0.5);
}

.highlight-num {
  color: var(--accent-cyan-vivid);
  font-weight: 600;
}

.pause-state-pill {
  font-size: 10.5px;
  color: #f59e0b;
  padding: 1px 6px;
  background: rgba(245, 158, 11, 0.12);
  border-radius: 4px;
  font-weight: 500;
}

.sub-right {
  display: flex;
  align-items: center;
  gap: 10px;
}

.hover-pause-toggle {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  user-select: none;
  padding: 3px 8px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 6px;
}

.hover-pause-toggle input {
  accent-color: var(--accent-cyan-vivid);
  cursor: pointer;
  margin: 0;
}

.view-mode-tabs {
  display: flex;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 7px;
  padding: 2px;
}

.view-tab-btn {
  padding: 3px 9px;
  font-size: var(--text-xs);
  border-radius: var(--radius-xs);
  background: transparent;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.view-tab-btn:hover {
  color: var(--text-primary);
}

.view-tab-btn.active {
  background: var(--accent-cyan-glow);
  color: var(--accent-cyan-vivid);
  font-weight: var(--weight-semibold);
}

/* 第二行：搜索 + 排序 + 状态/协议过滤（仅语义流/明细表） */
.toolbar-sub-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.search-box {
  flex: 1;
  min-width: 200px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  transition: all 0.2s ease;
}

.search-box:focus-within {
  background: rgba(255, 255, 255, 0.06);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent-cyan-vivid) 10%, transparent);
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

.btn-clear-search {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.4);
  cursor: pointer;
  font-size: 10px;
  padding: 2px 4px;
}

.btn-clear-search:hover {
  color: #fff;
}

/* 排序选择器：自绘触发按钮 */
.sort-picker {
  flex-shrink: 0;
}

.sort-trigger {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 8px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.8);
  font-size: 11.5px;
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

.sort-trigger:hover {
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.2);
  color: #fff;
}

.sort-trigger-arrow {
  opacity: 0.5;
  transition: transform 0.2s;
}

.sort-trigger-arrow.rotated {
  transform: rotate(180deg);
}

/* 升降序切换按钮（独立控件，点击区完整） */
.sort-order-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
}

.sort-order-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
}

/* 排序下拉弹层（Teleport 到 body；坐标/层级由内联 fixed 样式控制） */
.sort-menu {
  padding: 5px;
  background: rgba(18, 20, 28, 0.98);
  backdrop-filter: blur(16px);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 10px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.sort-menu-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 9px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: rgba(255, 255, 255, 0.75);
  font-size: 11.5px;
  cursor: pointer;
  text-align: left;
  transition: all 0.15s;
  white-space: nowrap;
}

.sort-menu-item:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}

.sort-menu-item.active {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent);
  color: var(--accent-cyan-vivid);
  font-weight: 600;
}

.sort-menu-check {
  color: var(--accent-cyan-vivid);
}

.sortmenu-enter-active, .sortmenu-leave-active {
  transition: opacity 0.18s, transform 0.18s cubic-bezier(0.4, 0, 0.2, 1);
}

.sortmenu-enter-from, .sortmenu-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.97);
}

/* 活跃/历史显示开关 */
.show-history-toggle {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  user-select: none;
  padding: 4px 9px;
  background: rgba(255, 255, 255, 0.035);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 7px;
  white-space: nowrap;
  transition: all 0.15s;
}

.show-history-toggle:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}

.show-history-toggle input {
  accent-color: var(--accent-cyan-vivid);
  cursor: pointer;
  margin: 0;
}

.filter-capsules {
  display: flex;
  align-items: center;
  gap: 6px;
}

.capsule {
  padding: 4px 9px;
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: rgba(255, 255, 255, 0.65);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
}

.capsule:hover {
  background: rgba(255, 255, 255, 0.08);
  color: #fff;
}

.capsule.active {
  background: rgba(255, 255, 255, 0.12);
  border-color: rgba(255, 255, 255, 0.25);
  color: #fff;
}

.capsule.proxied.active {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 15%, transparent);
  border-color: var(--accent-cyan-vivid);
  color: var(--accent-cyan-vivid);
}

.capsule.direct.active {
  background: rgba(16, 185, 129, 0.15);
  border-color: var(--accent-green);
  color: var(--accent-green);
}

.capsule.blocked.active {
  background: rgba(248, 113, 113, 0.15);
  border-color: #f87171;
  color: #f87171;
}

.proto-selector {
  display: flex;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 7px;
  padding: 2px;
}

.proto-btn {
  padding: 2px 7px;
  font-size: 11px;
  border-radius: 5px;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.5);
  cursor: pointer;
}

.proto-btn.active {
  background: rgba(255, 255, 255, 0.12);
  color: #fff;
  font-weight: 600;
}
</style>
