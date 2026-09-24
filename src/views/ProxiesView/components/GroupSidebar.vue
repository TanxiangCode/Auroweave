<script setup lang="ts">
import BaseIcon from "@/components/common/BaseIcon.vue";
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
  /** 自定义虚拟分组列表（本地匹配，tag 带 custom: 前缀） */
  customGroups?: ProxyGroup[];
  /** 自定义分组中含 unlock 匹配规则的名称集合（空态/时效提示用） */
  unlockRuleNames?: string[];
  /** 最近常用分组标签 */
  recentGroups: string[];
  /** 当前选中分组标签 */
  selectedGroupTag: string;
  /** 当前活跃路由链路分组标签集合 */
  routingGroupTags: Set<string>;
  /** 自定义分组规则总数（含无匹配节点的，用于空态提示） */
  customRulesCount?: number;
}>();

const emit = defineEmits<{
  select: [groupTag: string];
  'edit-group': [groupTag: string];
  'manage-regions': [];
  'edit-rule': [groupTag: string];
}>();;

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

/** 自定义分组显示列表（真实组 custom-、虚拟组 custom: 均去除内部前缀） */
const customGroupItems = computed(() =>
  (props.customGroups ?? []).map((g) => ({
    group: g,
    name: g.tag.replace(/^custom[:-]/, ""),
  }))
);

/** 是否存在 unlock 规则（空态文案区分"未检测"与"全失败"） */
const hasUnlockRule = computed(() => (props.unlockRuleNames ?? []).length > 0);

/** 地区分组列表（预计算地区徽标，避免模板每次渲染重复调用 3 次 getRegionBadge） */
const regionGroupsWithBadge = computed(() =>
  filteredRegionGroups.value.map((g) => ({ group: g, badge: getRegionBadge(g.tag) }))
);

function getGroupIcon(tag: string, type: string): string {
  if (tag === "proxy") return "Compass";
  if (tag === "auto") return "Zap";
  if (tag === "balance") return "Scale";
  if (type === "urltest") return "Globe";
  return "Folder";
}



interface RegionBadgeInfo {
  code: string;
  bg: string;
  color: string;
}

function getRegionBadge(tag: string): RegionBadgeInfo {
  const t = tag.toUpperCase();
  if (t.includes("HK") || tag.includes("香港")) {
    return { code: "HK", bg: "color-mix(in srgb, var(--accent-cyan-vivid) 15%, transparent)", color: "var(--accent-cyan-vivid)" };
  }
  if (t.includes("JP") || tag.includes("日本")) {
    return { code: "JP", bg: "rgba(255, 94, 98, 0.15)", color: "#ff5e62" };
  }
  // US 需按词边界匹配，避免误命中 RUSSIA / AUSTRALIA 等包含 "US" 子串的地区
  if (/(^|[^A-Z])US([^A-Z]|$)/.test(t) || tag.includes("美国") || tag.includes("美國")) {
    return { code: "US", bg: "rgba(79, 172, 254, 0.15)", color: "#4facfe" };
  }
  if (t.includes("TW") || tag.includes("台湾") || tag.includes("台灣")) {
    return { code: "TW", bg: "color-mix(in srgb, var(--accent-green) 15%, transparent)", color: "var(--accent-green)" };
  }
  if (t.includes("SG") || tag.includes("新加坡") || tag.includes("狮城")) {
    return { code: "SG", bg: "rgba(250, 112, 154, 0.15)", color: "#fa709a" };
  }
  if (t.includes("KR") || tag.includes("韩国") || tag.includes("韓國")) {
    return { code: "KR", bg: "rgba(56, 249, 215, 0.15)", color: "#38f9d7" };
  }
  if (/(^|[^A-Z])(UK|GB)([^A-Z]|$)/.test(t) || tag.includes("英国")) {
    return { code: "UK", bg: "rgba(161, 140, 209, 0.15)", color: "#a18cd1" };
  }
  if (/(^|[^A-Z])DE([^A-Z]|$)/.test(t) || tag.includes("德国")) {
    return { code: "DE", bg: "rgba(254, 207, 239, 0.15)", color: "#fecfef" };
  }
  if (/(^|[^A-Z])FR([^A-Z]|$)/.test(t) || tag.includes("法国")) {
    return { code: "FR", bg: "rgba(69, 162, 255, 0.15)", color: "#45a2ff" };
  }
  if (/(^|[^A-Z])CA([^A-Z]|$)/.test(t) || tag.includes("加拿大")) {
    return { code: "CA", bg: "rgba(255, 120, 117, 0.15)", color: "#ff7875" };
  }
  if (/(^|[^A-Z])AU([^A-Z]|$)/.test(t) || tag.includes("澳大利亚") || tag.includes("澳洲")) {
    return { code: "AU", bg: "rgba(255, 197, 61, 0.15)", color: "#ffc53d" };
  }

  // 提取首字母
  const letters = tag.replace(/[^a-zA-Z]/g, "").toUpperCase();
  const code = letters.length >= 2 ? letters.slice(0, 2) : tag.slice(0, 2).toUpperCase();
  return { code: code || "GL", bg: "rgba(255, 255, 255, 0.1)", color: "rgba(255, 255, 255, 0.85)" };
}

function getGroupTypeLabel(tag: string, type: string): string {
  if (tag === "proxy") return "主选择器";
  if (tag === "auto") return "自动优选";
  if (tag === "balance") return "独立优选";
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
                <BaseIcon :name="getGroupIcon(group.tag, group.type)" :size="15" class="group-type-icon" />
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
          v-for="{ group, badge } in regionGroupsWithBadge"
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
                <span class="region-code-badge" :style="{ background: badge.bg, color: badge.color }">{{ badge.code }}</span>
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

    <!-- 自定义分组（真实策略组 + 本地匹配虚拟分组；空规则态给出提示避免「找不到」） -->
    <div v-if="customGroupItems.length > 0 || (customRulesCount ?? 0) > 0" class="group-section">
      <div class="section-title-row">
        <span class="section-title">
          <BaseIcon name="Folder" :size="11" class="icon-gap" />
          自定义分组
        </span>
        <div class="section-actions">
          <span v-if="customGroupItems.length > 0" class="section-count-badge">{{ customGroupItems.length }}</span>
          <button class="btn-add-region" @click="emit('manage-regions')" title="管理自定义分组规则">
            <SvgIcon name="plus" :size="11" />
          </button>
        </div>
      </div>

      <div class="groups-list">
        <div v-if="customGroupItems.length === 0" class="custom-empty-hint">
          规则已保存，但暂无匹配节点（虚拟匹配需 proxy 主组节点池）——点击右上 + 编辑规则
        </div>
        <div
          v-for="{ group, name } in customGroupItems"
          :key="group.tag"
          class="group-item-wrapper"
        >
          <button
            class="group-card-item"
            :class="{
              active: group.tag === selectedGroupTag,
              'in-route': group.tag.startsWith('custom-') && routingGroupTags.has(group.tag)
            }"
            @click="emit('select', group.tag)"
          >
            <div class="group-header-info">
              <div class="group-name-wrapper">
                <BaseIcon name="GitFork" :size="14" class="group-type-icon" />
                <span class="group-name" :title="name">{{ name }}</span>
                <span
                  v-if="group.tag.startsWith('custom-') && routingGroupTags.has(group.tag)"
                  class="route-pulse-dot"
                  title="当前活跃出口链路成员"
                ></span>
              </div>
              <!-- 与地区分组复用同一徽章视觉，仅保留类型文案差异 -->
              <span v-if="group.tag.startsWith('custom-')" class="group-type-badge region">
                {{ group.type === 'urltest' ? '优选' : group.type === 'selector' ? '选择' : group.type }}
              </span>
              <span v-else class="group-type-badge region">本地</span>
            </div>
            <div class="group-footer-info">
              <span
                v-if="group.now"
                class="group-current-node"
                :class="{
                  'highlight-active': group.tag.startsWith('custom-') && routingGroupTags.has(group.tag)
                }"
                :title="group.tag.startsWith('custom-') ? `当前出口: ${group.now}` : group.now"
              >
                {{ group.now }}
              </span>
              <span v-else-if="group.proxies.length > 0" class="group-current-node">
                {{ group.proxies.length }} 个节点
              </span>
              <span v-else class="group-current-node muted">空分组</span>
            </div>
          </button>
          <button
            class="btn-group-config"
            title="编辑该分组规则"
            @click.stop="emit('edit-rule', group.tag)"
          >
            <SvgIcon name="edit" :size="11" />
          </button>
        </div>
      </div>

      <!-- unlock 规则时效提示（检测结果会过期，提醒重测） -->
      <div v-if="hasUnlockRule" class="custom-group-hint">
        解锁匹配分组依赖最近一次检测结果，节点变动或 IP 换段后建议重新检测
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

/* 自定义分组时效提示 */
.custom-group-hint {
  margin-top: var(--space-2);
  padding: var(--space-2) var(--space-3);
  font-size: 10px;
  line-height: 1.5;
  color: var(--text-tertiary);
  background: var(--layer-2);
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-sm);
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
  background: color-mix(in srgb, var(--accent-cyan-vivid) 10%, transparent);
  color: var(--accent-cyan);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
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


<style scoped>
.region-code-badge {
  font-size: 10px;
  font-weight: 700;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  padding: 1px 5px;
  border-radius: 4px;
  letter-spacing: 0.5px;
  flex-shrink: 0;
  line-height: 14px;
  border: 1px solid currentColor;
  opacity: 0.9;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.custom-empty-hint {
  padding: 10px 12px;
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-tertiary);
  background: var(--layer-1);
  border: 1px dashed var(--border-normal);
  border-radius: var(--radius-sm);
}
</style>
