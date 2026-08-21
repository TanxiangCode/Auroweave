<template>
  <div class="audit-filter-toolbar">
    <!-- 第一行：搜索与多维状态/协议过滤 -->
    <div class="toolbar-primary-row">
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

    <!-- 第二行：共匹配事件统计 + 悬停暂停开关 + 视图模式切换 (同在一行) -->
    <div class="toolbar-sub-row">
      <!-- 左侧：事件统计与暂停提示 -->
      <div class="sub-left">
        <span class="match-info">共匹配 <strong class="highlight-num">{{ matchCount }}</strong> 条语义化安全事件</span>
        <span class="pause-state-pill" v-if="isPaused">⏸️ 实时流已处于暂停状态</span>
      </div>

      <!-- 右侧：悬停暂停开关 + 视图切换 Tabs -->
      <div class="sub-right">
        <!-- 悬停自动暂停开关 -->
        <label class="hover-pause-toggle" title="鼠标移入列表时是否自动暂停实时流以方便查看">
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
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
defineProps<{
  searchQuery: string;
  statusFilter: "all" | "proxied" | "direct" | "blocked";
  protocolFilter: "all" | "tcp" | "udp";
  viewMode: "semantic" | "table" | "raw";
  autoPauseOnHover: boolean;
  totalCount: number;
  matchCount: number;
  isPaused: boolean;
}>();

defineEmits<{
  (e: "update:searchQuery", val: string): void;
  (e: "update:statusFilter", val: "all" | "proxied" | "direct" | "blocked"): void;
  (e: "update:protocolFilter", val: "all" | "tcp" | "udp"): void;
  (e: "update:viewMode", val: "semantic" | "table" | "raw"): void;
  (e: "update:autoPauseOnHover", val: boolean): void;
}>();
</script>

<style scoped>
.audit-filter-toolbar {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex-shrink: 0;
}

.toolbar-primary-row {
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
  border-color: rgba(0, 242, 254, 0.4);
  box-shadow: 0 0 0 2px rgba(0, 242, 254, 0.1);
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
  background: rgba(0, 242, 254, 0.15);
  border-color: #00f2fe;
  color: #00f2fe;
}

.capsule.direct.active {
  background: rgba(16, 185, 129, 0.15);
  border-color: #10b981;
  color: #10b981;
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

/* 第二行：同排布局 */
.toolbar-sub-row {
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
  color: #00f2fe;
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
  accent-color: #00f2fe;
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
  font-size: 11px;
  border-radius: 5px;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  transition: all 0.15s;
}

.view-tab-btn:hover {
  color: #fff;
}

.view-tab-btn.active {
  background: rgba(0, 242, 254, 0.16);
  color: #00f2fe;
  font-weight: 600;
}
</style>
