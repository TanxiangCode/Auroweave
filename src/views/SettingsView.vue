<script setup lang="ts">
/**
 * 设置视图 — 九大面板控制中心
 * 作者: TanXiang
 */
import { ref, computed, watch } from "vue";
import { useRoute } from "vue-router";
import GeneralPanel from "./panels/GeneralPanel.vue";
import DashboardPanel from "./panels/DashboardPanel.vue";
import SubscriptionPanel from "./panels/SubscriptionPanel.vue";
import RouteModePanel from "./panels/RouteModePanel.vue";
import DnsPanel from "./panels/DnsPanel.vue";
import SpeedtestPanel from "./panels/SpeedtestPanel.vue";
import TunPanel from "./panels/TunPanel/index.vue";
import AutomationPanel from "./panels/AutomationPanel.vue";
import HotkeyPanel from "./panels/HotkeyPanel.vue";
import PrivacyPanel from "./panels/PrivacyPanel/index.vue";
import AdvancedPanel from "./panels/AdvancedPanel.vue";

const route = useRoute();

type PanelKey =
  | "general"
  | "dashboard"
  | "subscription"
  | "routemode"
  | "dns"
  | "speedtest"
  | "tun"
  | "automation"
  | "hotkey"
  | "privacy"
  | "advanced";

const activePanel = ref<PanelKey>("general");
const highlightTarget = ref<string>("");
const searchKeyword = ref<string>("");

import BaseIcon from "@/components/common/BaseIcon.vue";

interface NavItem {
  key: PanelKey;
  icon: string;
  label: string;
  keywords: string[];
}

const navItems: NavItem[] = [
  { key: "general", icon: "Sliders", label: "通用设置", keywords: ["通用", "主题", "语言", "自启", "托盘", "最小化", "局域网", "共享", "allow lan", "general", "theme"] },
  { key: "dashboard", icon: "Layers", label: "首页显示", keywords: ["首页", "主页", "显示", "胶囊", "卡片", "出口", "ip", "节点", "延迟", "dashboard", "widget"] },
  { key: "subscription", icon: "Rss", label: "订阅管理", keywords: ["订阅", "节点", "更新", "导入", "sub", "node"] },
  { key: "routemode", icon: "GitFork", label: "代理模式", keywords: ["模式", "规则", "全局", "直连", "mode", "rule", "global"] },

  { key: "dns", icon: "Globe", label: "DNS 配置", keywords: ["dns", "域名解析", "nameserver", "fakeip", "doh"] },
  { key: "speedtest", icon: "Timer", label: "测速与解锁", keywords: ["测速", "延迟", "并发", "超时", "吞吐量", "优选", "解锁", "speedtest", "unlock", "gemini", "claude", "chatgpt"] },
  { key: "tun", icon: "Cpu", label: "TUN 网卡", keywords: ["tun", "虚拟网卡", "网关", "gvisor", "系统服务", "service"] },
  { key: "automation", icon: "Zap", label: "场景自动化", keywords: ["自动化", "wifi", "ssid", "切换", "auto"] },
  { key: "hotkey", icon: "Command", label: "全局热键", keywords: ["热键", "快捷键", "command palette", "hotkey"] },
  { key: "privacy", icon: "ShieldCheck", label: "隐私与日志", keywords: ["隐私", "日志", "清空", "导出", "log", "privacy"] },
  { key: "advanced", icon: "FlaskConical", label: "高级与内核", keywords: ["高级", "sing-box", "内核", "升级", "版本", "备份", "性能", "灾备", "advanced", "core", "update"] },
];

const filteredNavItems = computed(() => {
  const q = searchKeyword.value.trim().toLowerCase();
  if (!q) return navItems;
  return navItems.filter((item) => {
    return (
      item.label.toLowerCase().includes(q) ||
      item.keywords.some((k) => k.toLowerCase().includes(q))
    );
  });
});

// 从路由 query 同步面板与高亮目标。
//
// 注意：这里必须用 watch + { immediate: true }，不能用 onMounted ——
// Vue 的生命周期是「子组件 onMounted 先于父组件 onMounted」，
// 而 activePanel / highlightTarget 正是靠这两个值把子面板**切换出来**的。
// 若放在 onMounted，子面板挂载时读到的仍是初始空值，深链高亮永远不触发。
watch(
  () => route.query,
  (q) => {
    const p = q.panel as PanelKey | undefined;
    if (p && navItems.some((n) => n.key === p)) {
      activePanel.value = p;
    }
    const h = q.highlight as string | undefined;
    if (h) highlightTarget.value = h;
  },
  { immediate: true }
);
</script>

<template>
  <div class="settings-view">
    <!-- 左侧导航栏 -->
    <aside class="settings-sidebar glass-effect">
      <!-- 搜索过滤框 -->
      <div class="nav-search-box">
        <BaseIcon name="Search" :size="14" class="search-svg-icon" />
        <input
          v-model="searchKeyword"
          type="text"
          placeholder="搜索设置项 (如: 端口、内核、DNS)..."
        />
        <button v-if="searchKeyword" class="btn-clear" @click="searchKeyword = ''"><BaseIcon name="X" :size="12" /></button>
      </div>

      <!-- 导航列表 -->
      <nav class="nav-list">
        <button
          v-for="item in filteredNavItems"
          :key="item.key"
          class="nav-item"
          :class="{ active: activePanel === item.key }"
          @click="activePanel = item.key"
        >
          <span class="nav-icon">
            <BaseIcon :name="item.icon" :size="16" />
          </span>
          <span class="nav-label">{{ item.label }}</span>
        </button>


        <div v-if="filteredNavItems.length === 0" class="no-nav-match">
          未匹配到设置项
        </div>
      </nav>
    </aside>

    <!-- 右侧设置呈现区 -->
    <main class="settings-main glass-effect">
      <GeneralPanel v-if="activePanel === 'general'" />
      <DashboardPanel v-else-if="activePanel === 'dashboard'" />
      <SubscriptionPanel v-else-if="activePanel === 'subscription'" />
      <RouteModePanel v-else-if="activePanel === 'routemode'" />
      <DnsPanel v-else-if="activePanel === 'dns'" />
      <SpeedtestPanel v-else-if="activePanel === 'speedtest'" :highlight-target="highlightTarget" />
      <TunPanel v-else-if="activePanel === 'tun'" />
      <AutomationPanel v-else-if="activePanel === 'automation'" />
      <HotkeyPanel v-else-if="activePanel === 'hotkey'" />
      <PrivacyPanel v-else-if="activePanel === 'privacy'" />
      <AdvancedPanel
        v-else-if="activePanel === 'advanced'"
        :highlight-target="highlightTarget"
      />
    </main>
  </div>
</template>

<style scoped>
.settings-view {
  display: flex;
  height: 100%;
  gap: 12px;
  padding: var(--space-4) var(--space-5);
  overflow: hidden;
  box-sizing: border-box;
}

.settings-sidebar {
  width: 220px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  background: var(--surface-inset);
  border: 1px solid var(--border-normal);
  border-radius: 12px;
  flex-shrink: 0;
}

.nav-search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: 8px;
  transition: all 0.2s;
}

.nav-search-box:focus-within {
  background: var(--surface-hover);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 40%, transparent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent-cyan-vivid) 10%, transparent);
}

.search-svg-icon {
  width: 14px;
  height: 14px;
  color: var(--text-secondary);
  flex-shrink: 0;
  opacity: 0.7;
}


.nav-search-box input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: var(--text-primary);
  font-size: 11.5px;
}

.nav-search-box input::placeholder {
  color: var(--text-tertiary);
}

.btn-clear {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  cursor: pointer;
  font-size: 10px;
}

.nav-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  padding-right: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: 8px;
  background: transparent;
  border: 1px solid transparent;
  color: var(--text-secondary);
  font-size: 12.5px;
  cursor: pointer;
  transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
  text-align: left;
}

.nav-item:hover {
  background: var(--surface-hover);
  color: var(--text-primary);
}

.nav-item.active {
  background: color-mix(in srgb, var(--accent-cyan-vivid) 12%, transparent);
  border-color: color-mix(in srgb, var(--accent-cyan-vivid) 30%, transparent);
  color: var(--accent-cyan-vivid);
  font-weight: 600;
  box-shadow: 0 0 12px color-mix(in srgb, var(--accent-cyan-vivid) 15%, transparent);
}

.nav-icon {
  font-size: 14px;
}

.nav-label {
  flex: 1;
}

.no-nav-match {
  padding: 16px;
  text-align: center;
  font-size: 11.5px;
  color: var(--text-tertiary);
}

.settings-main {
  flex: 1;
  padding: 18px 22px;
  overflow-y: auto;
  background: var(--surface-inset);
  border: 1px solid var(--border-subtle);
  border-radius: 12px;
}
</style>
