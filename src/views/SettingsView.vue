<script setup lang="ts">
/**
 * 设置视图 — 九大面板控制中心
 * 作者: TanXiang
 */
import { ref, computed, onMounted } from "vue";
import { useRoute } from "vue-router";
import GeneralPanel from "./panels/GeneralPanel.vue";
import SubscriptionPanel from "./panels/SubscriptionPanel.vue";
import RouteModePanel from "./panels/RouteModePanel.vue";
import DnsPanel from "./panels/DnsPanel.vue";
import TunPanel from "./panels/TunPanel/index.vue";
import AutomationPanel from "./panels/AutomationPanel.vue";
import HotkeyPanel from "./panels/HotkeyPanel.vue";
import PrivacyPanel from "./panels/PrivacyPanel/index.vue";
import AdvancedPanel from "./panels/AdvancedPanel.vue";

const route = useRoute();

type PanelKey =
  | "general"
  | "subscription"
  | "routemode"
  | "dns"
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
  { key: "subscription", icon: "Layers", label: "订阅管理", keywords: ["订阅", "节点", "更新", "导入", "sub", "node"] },
  { key: "routemode", icon: "GitFork", label: "代理模式", keywords: ["模式", "规则", "全局", "直连", "mode", "rule", "global"] },
  { key: "dns", icon: "Globe", label: "DNS 配置", keywords: ["dns", "域名解析", "nameserver", "fakeip", "doh"] },
  { key: "tun", icon: "Cpu", label: "TUN 网卡", keywords: ["tun", "虚拟网卡", "网关", "gvisor", "系统服务", "service"] },
  { key: "automation", icon: "Zap", label: "场景自动化", keywords: ["自动化", "wifi", "ssid", "切换", "auto"] },
  { key: "hotkey", icon: "Command", label: "全局热键", keywords: ["热键", "快捷键", "command palette", "hotkey"] },
  { key: "privacy", icon: "ShieldCheck", label: "隐私与日志", keywords: ["隐私", "日志", "清空", "导出", "log", "privacy"] },
  { key: "advanced", icon: "FlaskConical", label: "高级与内核", keywords: ["高级", "sing-box", "内核", "升级", "版本", "测速", "延迟", "并发", "超时", "备份", "advanced", "speed", "core", "update"] },
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

onMounted(() => {
  if (route.query.panel) {
    const p = route.query.panel as PanelKey;
    if (navItems.some((n) => n.key === p)) {
      activePanel.value = p;
    }
  }
  if (route.query.highlight) {
    highlightTarget.value = route.query.highlight as string;
  }
});
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
      <SubscriptionPanel v-else-if="activePanel === 'subscription'" />
      <RouteModePanel v-else-if="activePanel === 'routemode'" />
      <DnsPanel v-else-if="activePanel === 'dns'" />
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
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 12px;
  flex-shrink: 0;
}

.nav-search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  transition: all 0.2s;
}

.nav-search-box:focus-within {
  background: rgba(255, 255, 255, 0.07);
  border-color: rgba(0, 242, 254, 0.4);
  box-shadow: 0 0 0 2px rgba(0, 242, 254, 0.1);
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
  color: #fff;
  font-size: 11.5px;
}

.nav-search-box input::placeholder {
  color: rgba(255, 255, 255, 0.35);
}

.btn-clear {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.4);
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
  color: rgba(255, 255, 255, 0.65);
  font-size: 12.5px;
  cursor: pointer;
  transition: all 0.15s cubic-bezier(0.4, 0, 0.2, 1);
  text-align: left;
}

.nav-item:hover {
  background: rgba(255, 255, 255, 0.05);
  color: #fff;
}

.nav-item.active {
  background: rgba(0, 242, 254, 0.12);
  border-color: rgba(0, 242, 254, 0.3);
  color: #00f2fe;
  font-weight: 600;
  box-shadow: 0 0 12px rgba(0, 242, 254, 0.15);
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
  color: rgba(255, 255, 255, 0.35);
}

.settings-main {
  flex: 1;
  padding: 18px 22px;
  overflow-y: auto;
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 12px;
}
</style>
