<script setup lang="ts">
/**
 * 分流配置中心 — 主视图
 * 作者: TanXiang
 *
 * 职责：应用级进程分流 (App-Matrix)、自定义域名/IP 规则分流、交互式拓扑画布 (Topology Canvas)
 */
import { ref, onMounted } from "vue";
import AppMatrixList from "@/components/routing/AppMatrixList.vue";
import CustomRulesList from "@/components/routing/CustomRulesList.vue";
import TopologyCanvas from "@/components/routing/TopologyCanvas.vue";
import { getAppRules, getCustomRules } from "@/api/ipc/routing";

const activeTab = ref<"matrix" | "rules" | "topology">("matrix");

const appRulesCount = ref(0);
const customRulesCount = ref(0);

async function refreshStats() {
  const [appRes, customRes] = await Promise.all([
    getAppRules(),
    getCustomRules(),
  ]);
  if (appRes.success && appRes.data) {
    appRulesCount.value = Object.keys(appRes.data).length;
  }
  if (customRes.success && customRes.data) {
    customRulesCount.value = customRes.data.filter((r) => r.enabled).length;
  }
}

onMounted(() => {
  refreshStats();
});
</script>

<template>
  <div class="routing-view">
    <!-- 顶部全景概览与 Tab 导航栏 -->
    <header class="routing-header glass-effect">
      <!-- 左侧：指标概览 -->
      <div class="header-metrics">
        <div class="metric-chip" title="已配置独立出站节点的应用进程">
          <span class="chip-icon"></span>
          <span class="chip-label">应用分流</span>
          <span class="chip-val text-cyan">{{ appRulesCount }}</span>
        </div>

        <div class="divider"></div>

        <div class="metric-chip" title="当前生效的自定义域名/IP 分流规则">
          <span class="chip-icon"></span>
          <span class="chip-label">自定义规则</span>
          <span class="chip-val text-green">{{ customRulesCount }}</span>
        </div>

        <div class="divider"></div>

        <div class="metric-chip" title="内置 GeoSite 与 GeoIP 大陆直连规则">
          <span class="chip-icon">🇨🇳</span>
          <span class="chip-label">GeoSite 大陆直连</span>
          <span class="chip-val">已内置</span>
        </div>
      </div>

      <!-- 右侧：极光 Tab 切换组 -->
      <div class="tab-capsules">
        <button
          class="tab-capsule-btn"
          :class="{ active: activeTab === 'matrix' }"
          @click="activeTab = 'matrix'"
        >
          应用分流
        </button>

        <button
          class="tab-capsule-btn"
          :class="{ active: activeTab === 'rules' }"
          @click="activeTab = 'rules'"
        >
          域名规则
        </button>

        <button
          class="tab-capsule-btn"
          :class="{ active: activeTab === 'topology' }"
          @click="activeTab = 'topology'"
        >
          拓扑画布
        </button>
      </div>
    </header>

    <!-- 主视图呈现区 -->
    <main class="routing-main">
      <KeepAlive>
        <AppMatrixList v-if="activeTab === 'matrix'" />
        <CustomRulesList v-else-if="activeTab === 'rules'" />
        <TopologyCanvas v-else />
      </KeepAlive>
    </main>
  </div>
</template>

<style scoped>
.routing-view {
  padding: var(--space-4) var(--space-5);
  height: 100%;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 10px;
  box-sizing: border-box;
}

.routing-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 14px;
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 12px;
  gap: 12px;
  flex-shrink: 0;
}

.header-metrics {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.metric-chip {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
}

.chip-icon {
  font-size: 13px;
}

.chip-label {
  color: rgba(255, 255, 255, 0.55);
  font-size: 11px;
}

.chip-val {
  font-weight: 700;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  color: rgba(255, 255, 255, 0.9);
}

.text-cyan { color: #00f2fe !important; }
.text-green { color: #10b981 !important; }

.divider {
  width: 1px;
  height: 14px;
  background: rgba(255, 255, 255, 0.1);
}

.tab-capsules {
  display: flex;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  padding: 2px;
}

.tab-capsule-btn {
  padding: 4px 12px;
  font-size: 11.5px;
  border-radius: 6px;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.6);
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  font-weight: 500;
}

.tab-capsule-btn:hover {
  color: #fff;
}

.tab-capsule-btn.active {
  background: rgba(0, 242, 254, 0.16);
  color: #00f2fe;
  font-weight: 600;
  box-shadow: 0 0 10px rgba(0, 242, 254, 0.2);
}

.routing-main {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
</style>
