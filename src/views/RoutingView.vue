<script setup lang="ts">
/**
 * 分流配置视图（简单 App-Matrix 矩阵模式 / 高级拓扑预览）
 * 作者: TanXiang
 */
import { ref } from "vue";
import AppMatrixList from "@/components/routing/AppMatrixList.vue";

const activeTab = ref<"matrix" | "topology">("matrix");
</script>

<template>
  <div class="routing-view">
    <header class="routing-header">
      <div class="title-area">
        <h1>🛠️ 分流矩阵 · App-Matrix</h1>
        <p class="subtitle">为独立应用进程绑定专属出站节点，实现精准流量导流</p>
      </div>

      <div class="tab-controls">
        <button
          class="tab-btn"
          :class="{ active: activeTab === 'matrix' }"
          @click="activeTab = 'matrix'"
        >
          📱 矩阵模式 (App-Matrix)
        </button>
        <button
          class="tab-btn disabled"
          title="拓扑图链路由模块 J 解锁"
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
  padding: 24px;
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.routing-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.title-area h1 {
  font-size: 20px;
  font-weight: 700;
}

.subtitle {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
  margin-top: 4px;
}

.tab-controls {
  display: flex;
  gap: 8px;
  background: rgba(255, 255, 255, 0.04);
  padding: 4px;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.tab-btn {
  padding: 6px 14px;
  border-radius: 8px;
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.6);
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s ease;
}

.tab-btn.active {
  background: rgba(0, 242, 254, 0.15);
  color: #fff;
  font-weight: 600;
}

.tab-btn.disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.routing-body {
  flex: 1;
  overflow: hidden;
}
</style>
