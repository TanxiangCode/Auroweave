<script setup lang="ts">
/**
 * 代理节点视图
 * 作者: TanXiang
 */
import { onMounted, ref } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { storeToRefs } from "pinia";

const proxyStore = useProxyStore();
const { groups, loading, error } = storeToRefs(proxyStore);
const selectedGroupTag = ref<string>("");

onMounted(async () => {
  await proxyStore.fetchGroups();
  if (groups.value.length > 0) {
    selectedGroupTag.value = groups.value[0].tag;
    await proxyStore.fetchGroupNodes(selectedGroupTag.value);
  }
});

async function handleGroupSelect(groupTag: string) {
  selectedGroupTag.value = groupTag;
  await proxyStore.fetchGroupNodes(groupTag);
}

async function handleNodeSelect(nodeTag: string) {
  if (!selectedGroupTag.value) return;
  await proxyStore.selectNode(selectedGroupTag.value, nodeTag);
}
</script>

<template>
  <div class="proxies-view">
    <header class="proxies-header">
      <h1>🌐 代理节点</h1>
      <button class="btn-refresh" @click="proxyStore.fetchGroups">🔄 刷新</button>
    </header>

    <div v-if="loading" class="state-tip">
      ⏳ 正在获取代理节点...
    </div>

    <div v-else-if="error" class="state-tip error">
      ⚠️ {{ error }}
    </div>

    <div v-else-if="groups.length === 0" class="state-tip">
      <div class="placeholder-icon">🌐</div>
      <p>未检出到已运行的代理节点组。</p>
      <p class="sub-tip">请先在「设置」中导入订阅并启动 sing-box 核心。</p>
    </div>

    <div v-else class="proxies-container">
      <!-- 组 Tabs -->
      <div class="group-tabs">
        <button
          v-for="group in groups"
          :key="group.tag"
          class="group-tab"
          :class="{ active: group.tag === selectedGroupTag }"
          @click="handleGroupSelect(group.tag)"
        >
          <span class="group-name">{{ group.tag }}</span>
          <span v-if="group.now" class="group-now">({{ group.now }})</span>
        </button>
      </div>

      <!-- 节点网格 -->
      <div class="node-grid">
        <div
          v-for="node in proxyStore.nodeMap.get(selectedGroupTag) ?? []"
          :key="node.tag"
          class="node-card"
          :class="{ active: node.is_active }"
          @click="handleNodeSelect(node.tag)"
        >
          <div class="node-header">
            <span class="node-tag">{{ node.tag }}</span>
            <span class="node-type">{{ node.type }}</span>
          </div>

          <div class="node-footer">
            <span class="status-indicator" :class="{ active: node.is_active }">
              {{ node.is_active ? '● 当前选中' : '○ 点击选择' }}
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.proxies-view {
  padding: 48px 24px 24px 24px;
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.proxies-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.proxies-header h1 {
  font-size: var(--text-xl);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.btn-refresh {
  padding: var(--space-2) var(--space-4);
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  font-size: var(--text-xs);
  cursor: pointer;
}

.state-tip {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-10);
  gap: var(--space-2);
  color: var(--text-secondary);
}

.state-tip.error {
  color: var(--accent-red);
}

.sub-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.group-tabs {
  display: flex;
  gap: var(--space-2);
  overflow-x: auto;
  padding-bottom: var(--space-2);
}

.group-tab {
  padding: var(--space-2) var(--space-4);
  background: var(--layer-1);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  white-space: nowrap;
  display: flex;
  gap: var(--space-2);
  align-items: center;
}

.group-tab.active {
  background: var(--layer-2);
  border-color: var(--accent-blue);
  color: var(--text-primary);
  font-weight: var(--weight-semibold);
}

.group-now {
  font-size: var(--text-xs);
  color: var(--accent-cyan);
}

.node-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--space-4);
}

.node-card {
  padding: var(--space-4);
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  transition: all var(--duration-fast);
}

.node-card:hover {
  border-color: var(--border-strong);
  transform: translateY(-2px);
}

.node-card.active {
  border-color: var(--accent-green);
  box-shadow: 0 0 12px rgba(16, 185, 129, 0.2);
}

.node-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.node-tag {
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-type {
  font-size: 10px;
  padding: 2px 6px;
  background: var(--layer-2);
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  text-transform: uppercase;
}

.node-footer {
  display: flex;
  justify-content: flex-end;
}

.status-indicator {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.status-indicator.active {
  color: var(--accent-green);
  font-weight: var(--weight-medium);
}
</style>
