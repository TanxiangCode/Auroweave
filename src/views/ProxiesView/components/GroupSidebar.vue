<script setup lang="ts">
/**
 * 左栏分组选择器
 * 作者: TanXiang
 *
 * 包含：最近常用分组、系统分组（主策略组）、地区分组
 */
import SvgIcon from "@/components/common/SvgIcon.vue";
import type { ProxyGroup } from "@/types";

defineProps<{
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
</script>

<template>
  <aside class="sidebar-groups glass-effect">
    <!-- 最近常用快捷标签 -->
    <div v-if="recentGroups.length > 1" class="recent-section">
      <span class="section-title">
        <SvgIcon name="clock" :size="12" class="icon-gap" />
        最近常用
      </span>
      <div class="recent-list">
        <button
          v-for="tag in recentGroups"
          :key="tag"
          class="recent-item"
          :class="{ active: tag === selectedGroupTag }"
          @click="emit('select', tag)"
        >
          {{ tag }}
        </button>
      </div>
    </div>

    <!-- 系统分组 -->
    <span class="section-title">
      <SvgIcon name="folder" :size="12" class="icon-gap" />
      主策略组
    </span>
    <div class="groups-list">
      <div
        v-for="group in systemGroups"
        :key="group.tag"
        class="group-item-wrapper"
      >
        <button
          class="group-item"
          :class="{
            active: group.tag === selectedGroupTag,
            'in-route': routingGroupTags.has(group.tag)
          }"
          @click="emit('select', group.tag)"
        >
          <div class="group-header-info">
            <div class="group-name-wrapper">
              <span v-if="routingGroupTags.has(group.tag)" class="route-dot" title="当前活跃出口链路成员"></span>
              <span class="group-name">{{ group.tag }}</span>
            </div>
            <span class="group-badge">{{ group.type }}</span>
          </div>
          <span
            v-if="group.now"
            class="group-current-node"
            :class="{ 'highlight-now': routingGroupTags.has(group.tag) }"
          >
            {{ group.now }}
          </span>
        </button>
        <button
          v-if="group.tag !== 'proxy'"
          class="btn-group-edit"
          @click.stop="emit('edit-group', group.tag)"
          title="编辑分组配置"
        >
          <SvgIcon name="edit" :size="10" />
        </button>
      </div>
    </div>

    <!-- 地区分组 -->
    <template>
      <div class="section-title-row">
        <span class="section-title">
          <SvgIcon name="globe" :size="12" class="icon-gap" />
          地区分组
        </span>
        <button class="btn-add-region" @click="emit('manage-regions')" title="管理自定义区域">
          <SvgIcon name="plus" :size="12" />
        </button>
      </div>
      <div class="groups-list">
        <button
          v-for="group in regionGroups"
          :key="group.tag"
          class="group-item"
          :class="{
            active: group.tag === selectedGroupTag,
            'in-route': routingGroupTags.has(group.tag)
          }"
          @click="emit('select', group.tag)"
        >
          <div class="group-header-info">
            <div class="group-name-wrapper">
              <span v-if="routingGroupTags.has(group.tag)" class="route-dot" title="当前活跃出口链路成员"></span>
              <span class="group-name">{{ group.tag }}</span>
            </div>
            <span class="group-badge">{{ group.type }}</span>
          </div>
          <span
            v-if="group.now"
            class="group-current-node"
            :class="{ 'highlight-now': routingGroupTags.has(group.tag) }"
          >
            {{ group.now }}
          </span>
        </button>
      </div>
    </template>
  </aside>
</template>

<style scoped>
.sidebar-groups {
  width: 220px;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  overflow-y: auto;
  flex-shrink: 0;
}

.section-title {
  display: flex;
  align-items: center;
  font-size: var(--text-xs);
  font-weight: var(--weight-bold);
  color: var(--text-tertiary);
  text-transform: uppercase;
  margin-top: var(--space-2);
}

.recent-list,
.groups-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.recent-item,
.group-item {
  display: flex;
  flex-direction: column;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  border: 1px solid transparent;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  text-align: left;
  transition: all var(--duration-fast) var(--ease-out);
}

.recent-item:hover,
.group-item:hover {
  background: var(--border-subtle);
  color: var(--text-primary);
}

.recent-item.active,
.group-item.active {
  background: var(--accent-blue-glow);
  border-color: var(--accent-blue);
  color: var(--accent-blue);
  font-weight: var(--weight-semibold);
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
  gap: var(--space-2);
  min-width: 0;
  flex: 1;
}

.route-dot {
  width: 6px;
  height: 6px;
  background-color: var(--accent-green);
  border-radius: 50%;
  box-shadow: 0 0 8px var(--accent-green);
  flex-shrink: 0;
  animation: pulse-green 2s infinite;
}

@keyframes pulse-green {
  0% { box-shadow: 0 0 0 0 rgba(52, 211, 153, 0.7); }
  70% { box-shadow: 0 0 0 6px rgba(52, 211, 153, 0); }
  100% { box-shadow: 0 0 0 0 rgba(52, 211, 153, 0); }
}

.group-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-badge {
  font-size: var(--text-xs);
  padding: 1px var(--space-1);
  background: var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  font-family: var(--font-mono);
}

.group-current-node {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-top: 2px;
  transition: color var(--duration-fast);
}

.group-current-node.highlight-now {
  color: var(--accent-cyan);
  font-weight: var(--weight-medium);
}

/* 分组项包装器（带编辑按钮） */
.group-item-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.group-item-wrapper .group-item {
  flex: 1;
}

.btn-group-edit {
  position: absolute;
  right: var(--space-1);
  top: 50%;
  transform: translateY(-50%);
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--layer-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  opacity: 0;
  transition: all var(--duration-fast) var(--ease-out);
}

.group-item-wrapper:hover .btn-group-edit {
  opacity: 1;
}

.btn-group-edit:hover {
  color: var(--accent-blue);
  border-color: var(--accent-blue);
}

/* 分区标题行（带操作按钮） */
.section-title-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.btn-add-region {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease-out);
}

.btn-add-region:hover {
  color: var(--accent-blue);
  border-color: var(--accent-blue);
}

/* 图标间距工具类 */
.icon-gap {
  margin-right: 4px;
}
</style>
