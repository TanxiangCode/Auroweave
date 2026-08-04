<script setup lang="ts">
/**
 * 分流配置视图（简单 App-Matrix 矩阵模式 / 高级拓扑预览）
 * 作者: TanXiang
 */
import { ref } from "vue";
import AppMatrixList from "@/components/routing/AppMatrixList.vue";
import { useRouter } from "vue-router";
import { useToast } from "@/composables/useToast";

const activeTab = ref<"matrix" | "topology">("matrix");
const router = useRouter();
const toast = useToast();

function handleTopologyClick() {
  toast.info("拓扑画布尚未启用", "正在为您跳转到设置页，请开启「启用拓扑图功能」");
  setTimeout(() => {
    router.push({ path: "/settings", query: { panel: "advanced", highlight: "topology" } });
  }, 1200);
}
</script>

<template>
  <div class="routing-view">
    <header class="page-header">
      <div class="title-area">
        <h1>🛠️ 分流矩阵 · App-Matrix</h1>
        <p class="subtitle">为独立应用进程绑定专属出站节点，实现精准流量导流</p>
      </div>
      <div class="tab-group">
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'matrix' }"
          @click="activeTab = 'matrix'"
        >
          📱 矩阵模式 (App-Matrix)
        </button>
        <button
          class="tab-btn disabled"
          title="点击前往设置开启拓扑图"
          @click="handleTopologyClick"
        >
          🔒 拓扑画布 (Topology Canvas)
        </button>
      </div>
    </header>

    <main class="routing-body">
      <AppMatrixList v-if="activeTab === 'matrix'" />
    </main>
  </div>
</template>

<style scoped>
.routing-view {
  padding: var(--space-5);
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.routing-body {
  flex: 1;
  overflow: hidden;
}
</style>
