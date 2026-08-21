<script setup lang="ts">
/**
 * 左栏分组选择器 (升级版)
 * 作者: TanXiang
 *
 * 包含：分组快速过滤、最近常用分组、系统主策略组、地区分组、专属图标与活跃链路高亮
 */
import { ref, computed } from "vue";
import SvgIcon from "@/components/common/SvgIcon.vue";
import type { ProxyGroup } from "@/types";

const props = defineProps<{
  /** 系统分组列表 */
  systemGroups: ProxyGroup[];
  /** 地区分组列表 */
  regionGroups: ProxyGroup[];
  /** 最近常用分组标签 */
  recentGroups: string[];
  /** 当前选中分组标签 */
  selectedGroupTag: string;
  /** 当前活跃路由链路分组标签集合 */
  routingGroupTags: Set<string>;
}>();

const emit = defineEmits<{
  select: [groupTag: string];
  'edit-group': [groupTag: string];
  'manage-regions': [];
}>();

const groupSearch = ref("");

const filteredSystemGroups = computed(() => {
  if (!groupSearch.value.trim()) return props.systemGroups;
  const kw = groupSearch.value.toLowerCase();
  return props.systemGroups.filter(g => g.tag.toLowerCase().includes(kw) || g.type.toLowerCase().includes(kw));
});

const filteredRegionGroups = computed(() => {
  if (!groupSearch.value.trim()) return props.regionGroups;
  const kw = groupSearch.value.toLowerCase();
  return props.regionGroups.filter(g => g.tag.toLowerCase().includes(kw));
});

function getGroupIcon(tag: string, type: string): string {
  if (tag === "proxy") return "P";
  if (tag === "auto") return "A";
  if (tag === "balance") return "B";
  if (type === "urltest") return "U";
  return "G";
}


function getGroupTypeLabel(tag: string, type: string): string {
  if (tag === "proxy") return "主选择器";
  if (tag === "auto") return "自动优选";
  if (tag === "balance") return "负载均衡";
  if (type === "urltest") return "URLTest";
  return type.toUpperCase();
}
</script>

<template>
  <aside class="sidebar-groups glass-effect">
    <!-- 顶部搜索框 (当分组数较多时自动提供快速定位) -->
    <div class="sidebar-search-box">
      <SvgIcon name="search" :size="12" class="search-icon" />
      <input
        v-model="groupSearch"
        type="text"
        placeholder="搜索策略组..."
        class="group-search-input"
      />
      <button v-if="groupSearch" class="search-clear-btn" @click="groupSearch = ''">×</button>
    </div>

    <!-- 最近常用快捷标签 -->
    <div v-if="recentGroups.length > 1 && !groupSearch" class="recent-section">
      <span class="section-title">
        <SvgIcon name="clock" :size="11" class="icon-gap" />
        最近常用
      </span>
      <div class="recent-tags-row">
        <button
          v-for="tag in recentGroups"
          :key="tag"
          class="recent-tag-capsule"
          :class="{ active: tag === selectedGroupTag }"
          @click="emit('select', tag)"
          :title="`快速切换到 ${tag}`"
        >
          {{ tag }}
        </button>
      </div>
    </div>

    <!-- 系统主策略组 -->
    <div class="group-section">
      <div class="section-title-row">
        <span class="section-title">
          <SvgIcon name="folder" :size="11" class="icon-gap" />
          主策略组
        </span>
        <span class="section-count-badge">{{ filteredSystemGroups.length }}</span>
      </div>

      <div class="groups-list">
        <div
          v-for="group in filteredSystemGroups"
          :key="group.tag"
          class="group-item-wrapper"
        >
          <button
            class="group-card-item"
            :class="{
              active: group.tag === selectedGroupTag,
              'in-route': routingGroupTags.has(group.tag)
            }"
            @click="emit('select', group.tag)"
          >
            <div class="group-header-info">
              <div class="group-name-wrapper">
                <span class="group-type-emoji">{{ getGroupIcon(group.tag, group.type) }}</span>
                <span class="group-name" :title="group.tag">{{ group.tag }}</span>
                <span v-if="routingGroupTags.has(group.tag)" class="route-pulse-dot" title="当前活跃出口链路成员"></span>
              </div>
              <span class="group-type-badge" :class="group.tag">{{ getGroupTypeLabel(group.tag, group.type) }}</span>
            </div>

            <div class="group-footer-info">
              <span
                v-if="group.now"
                class="group-current-node"
                :class="{ 'highlight-active': routingGroupTags.has(group.tag) }"
                :title="`当前出口: ${group.now}`"
              >
                {{ group.now }}
              </span>
              <span v-else class="group-current-node muted">测速优选中...</span>
            </div>
          </button>

          <!-- 编辑策略组按钮 -->
          <button
            v-if="group.tag !== 'proxy'"
            class="btn-group-config"
            @click.stop="emit('edit-group', group.tag)"
            title="配置测速参数"
          >
            <SvgIcon name="edit" :size="11" />
          </button>
        </div>
      </div>
    </div>

    <!-- 地区与自定义分组 -->
    <div class="group-section">
      <div class="section-title-row">
        <span class="section-title">
          <SvgIcon name="globe" :size="11" class="icon-gap" />
          地区分组
        </span>
        <div class="section-actions">
          <span class="section-count-badge">{{ filteredRegionGroups.length }}</span>
          <button class="btn-add-region" @click="emit('manage-regions')" title="管理自定义区域与分流规则">
            <SvgIcon name="plus" :size="11" />
          </button>
        </div>
      </div>

      <div class="groups-list">
        <div
          v-for="group in filteredRegionGroups"
          :key="group.tag"
          class="group-item-wrapper"
        >
          <button
            class="group-card-item"
            :class="{
              active: group.tag === selectedGroupTag,
              'in-route': routingGroupTags.has(group.tag)
            }"
            @click="emit('select', group.tag)"
          >
            <div class="group-header-info">
              <div class="group-name-wrapper">
                <span class="group-name" :title="group.tag">{{ group.tag }}</span>
                <span v-if="routingGroupTags.has(group.tag)" class="route-pulse-dot" title="当前活跃出口链路成员"></span>
              </div>
              <span class="group-type-badge region">AUTO</span>
            </div>


            <div class="group-footer-info">
              <span
                v-if="group.now"
                class="group-current-node"
                :class="{ 'highlight-active': routingGroupTags.has(group.tag) }"
                :title="`当前出口: ${group.now}`"
              >
                {{ group.now }}
              </span>
              <span v-else class="group-current-node muted">测速中...</span>
            </div>
          </button>

          <button
            class="btn-group-config"
            @click.stop="emit('edit-group', group.tag)"
            title="配置测速参数"
          >
            <SvgIcon name="edit" :size="11" />
          </button>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.sidebar-groups {
  width: 240px;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-3);
  overflow-y: auto;
  flex-shrink: 0;
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  background: var(--layer-1);
}

/* 快速检索 */
.sidebar-search-box {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 4px 8px;
  transition: all var(--duration-fast);
}

.sidebar-search-box:focus-within {
  border-color: var(--accent-cyan);
  box-shadow: 0 0 6px var(--accent-cyan-glow);
}

.search-icon {
  color: var(--text-tertiary);
  flex-shrink: 0;
}

.group-search-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  font-size: var(--text-xs);
  color: var(--text-primary);
  min-width: 0;
}

.group-search-input::placeholder {
  color: var(--text-tertiary);
}

.search-clear-btn {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  font-size: 14px;
  cursor: pointer;
  padding: 0 2px;
}

/* 常用标签胶囊 */
.recent-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.recent-tags-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.recent-tag-capsule {
  padding: 2px 8px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full);
  font-size: 10px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.recent-tag-capsule:hover {
  background: var(--layer-3);
  color: var(--text-primary);
  border-color: var(--border-normal);
}

.recent-tag-capsule.active {
  background: var(--accent-cyan-glow);
  border-color: var(--accent-cyan);
  color: var(--accent-cyan);
  font-weight: var(--weight-bold);
}

/* 分区容器 */
.group-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.section-title-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 4px;
}

.section-title {
  display: flex;
  align-items: center;
  font-size: 11px;
  font-weight: var(--weight-bold);
  color: var(--text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.section-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.section-count-badge {
  font-size: 10px;
  padding: 0 5px;
  background: var(--layer-2);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.btn-add-region {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-add-region:hover {
  color: var(--accent-cyan);
  border-color: var(--accent-cyan);
}

/* 分组列表与卡片 */
.groups-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.group-item-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.group-card-item {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 10px;
  background: var(--layer-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  cursor: pointer;
  text-align: left;
  transition: all var(--duration-fast) cubic-bezier(0.16, 1, 0.3, 1);
}

.group-card-item:hover {
  background: var(--layer-3);
  border-color: var(--border-normal);
  transform: translateY(-1px);
}

.group-card-item.active {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--text-primary);
  box-shadow: 0 0 10px rgba(79, 140, 255, 0.15);
}

.group-card-item.in-route {
  border-color: rgba(52, 211, 153, 0.4);
}

.group-card-item.in-route.active {
  border-color: var(--accent-cyan);
  background: var(--accent-cyan-glow);
  box-shadow: 0 0 12px var(--accent-cyan-glow);
}

.group-header-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
}

.group-name-wrapper {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex: 1;
}

.group-type-emoji {
  font-size: 13px;
  flex-shrink: 0;
}

.group-name {
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-primary);
}

.route-pulse-dot {
  width: 6px;
  height: 6px;
  background-color: var(--accent-green);
  border-radius: 50%;
  box-shadow: 0 0 6px var(--accent-green);
  flex-shrink: 0;
  animation: pulse-glow 2s infinite;
}

@keyframes pulse-glow {
  0% { box-shadow: 0 0 0 0 rgba(52, 211, 153, 0.7); }
  70% { box-shadow: 0 0 0 5px rgba(52, 211, 153, 0); }
  100% { box-shadow: 0 0 0 0 rgba(52, 211, 153, 0); }
}

.group-type-badge {
  font-size: 9px;
  padding: 1px 4px;
  border-radius: var(--radius-xs);
  font-family: var(--font-mono);
  background: var(--layer-3);
  color: var(--text-tertiary);
  border: 1px solid var(--border-subtle);
  text-transform: uppercase;
  flex-shrink: 0;
}

.group-type-badge.proxy {
  background: rgba(79, 140, 255, 0.1);
  color: var(--accent-blue);
  border-color: rgba(79, 140, 255, 0.3);
}

.group-type-badge.auto,
.group-type-badge.region {
  background: rgba(0, 242, 254, 0.1);
  color: var(--accent-cyan);
  border-color: rgba(0, 242, 254, 0.3);
}

.group-type-badge.balance {
  background: rgba(245, 158, 11, 0.1);
  color: var(--accent-orange);
  border-color: rgba(245, 158, 11, 0.3);
}

.group-footer-info {
  display: flex;
  align-items: center;
  width: 100%;
}

.group-current-node {
  font-size: 11px;
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-current-node.highlight-active {
  color: var(--accent-cyan);
  font-weight: var(--weight-medium);
}

.group-current-node.muted {
  font-style: italic;
  opacity: 0.7;
}

.btn-group-config {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  width: 20px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--layer-3);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  opacity: 0;
  transition: all var(--duration-fast);
}

.group-item-wrapper:hover .btn-group-config {
  opacity: 1;
}

.btn-group-config:hover {
  color: var(--accent-cyan);
  border-color: var(--accent-cyan);
}

.icon-gap {
  margin-right: 4px;
}
</style>

