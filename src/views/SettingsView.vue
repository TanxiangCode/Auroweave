<script setup lang="ts">
/**
 * 设置视图 — 九大面板控制中心
 * 作者: TanXiang
 */
import { ref, onMounted } from "vue";
import { useRoute } from "vue-router";
import GeneralPanel from "./panels/GeneralPanel.vue";
import SubscriptionPanel from "./panels/SubscriptionPanel.vue";
import RouteModePanel from "./panels/RouteModePanel.vue";
import DnsPanel from "./panels/DnsPanel.vue";
import TunPanel from "./panels/TunPanel.vue";
import AutomationPanel from "./panels/AutomationPanel.vue";
import HotkeyPanel from "./panels/HotkeyPanel.vue";
import PrivacyPanel from "./panels/PrivacyPanel.vue";
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

const navItems: Array<{ key: PanelKey; icon: string; label: string }> = [
  { key: "general", icon: "⚙️", label: "通用设置" },
  { key: "subscription", icon: "📦", label: "订阅管理" },
  { key: "routemode", icon: "🔀", label: "代理模式" },
  { key: "dns", icon: "📡", label: "DNS 配置" },
  { key: "tun", icon: "🔌", label: "TUN 网卡" },
  { key: "automation", icon: "⚡", label: "场景自动化" },
  { key: "hotkey", icon: "⌨️", label: "全局热键" },
  { key: "privacy", icon: "🛡️", label: "隐私与日志" },
  { key: "advanced", icon: "🧪", label: "高级与性能" },
];

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
    <!-- 侧边栏导航 -->
    <aside class="settings-sidebar glass-effect">
      <nav class="nav-list">
        <button
          v-for="item in navItems"
          :key="item.key"
          class="nav-item"
          :class="{ active: activePanel === item.key }"
          @click="activePanel = item.key"
        >
          <span class="nav-icon">{{ item.icon }}</span>
          <span class="nav-label">{{ item.label }}</span>
        </button>
      </nav>
    </aside>

    <!-- 主面板渲染区域 -->
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
  gap: 16px;
  padding: 20px;
  overflow: hidden;
}

.settings-sidebar {
  width: 220px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  padding: 16px;
}

.nav-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 10px;
  background: transparent;
  border: 1px solid transparent;
  color: rgba(255, 255, 255, 0.65);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.15s ease;
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
}

.nav-icon {
  font-size: 16px;
}

.settings-main {
  flex: 1;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  padding: 24px;
  overflow-y: auto;
}
</style>
