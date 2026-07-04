<script setup lang="ts">
/**
 * 设置视图 — 包含订阅管理面板
 * 作者: TanXiang
 */
import { ref, onMounted } from "vue";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { storeToRefs } from "pinia";

const subStore = useSubscriptionStore();
const { subscriptions, importing, importError } = storeToRefs(subStore);

const nameInput = ref("");
const urlInput = ref("");
const successMsg = ref<string | null>(null);

onMounted(() => {
  subStore.fetchAll();
});

async function handleImport() {
  if (!nameInput.value.trim() || !urlInput.value.trim()) return;
  successMsg.value = null;

  const res = await subStore.importSub(nameInput.value.trim(), urlInput.value.trim());
  if (res.success) {
    successMsg.value = `成功导入订阅 "${nameInput.value}"，共 ${res.data?.node_count ?? 0} 个节点！`;
    nameInput.value = "";
    urlInput.value = "";
  }
}

async function handleRemove(id: string) {
  await subStore.removeSub(id);
}
</script>

<template>
  <div class="settings-view">
    <header class="settings-header">
      <h1>⚙️ 应用设置与订阅</h1>
    </header>

    <div class="settings-grid">
      <!-- 订阅导入卡片 -->
      <div class="panel-card">
        <h2>📥 导入新订阅</h2>
        <p class="panel-desc">支持 Clash (YAML)、V2Ray (Base64) 及 Sing-box (JSON) 格式订阅链接</p>

        <form class="import-form" @submit.prevent="handleImport">
          <div class="field-group">
            <label for="sub-name">订阅名称</label>
            <input
              id="sub-name"
              v-model="nameInput"
              type="text"
              placeholder="例如：极客机场 主订阅"
              required
            />
          </div>

          <div class="field-group">
            <label for="sub-url">订阅 URL</label>
            <input
              id="sub-url"
              v-model="urlInput"
              type="url"
              placeholder="https://example.com/api/v1/client/subscribe?token=..."
              required
            />
          </div>

          <button type="submit" class="btn-primary" :disabled="importing">
            <span v-if="importing">⏳ 正在解析并生成配置...</span>
            <span v-else>🚀 开始导入</span>
          </button>
        </form>

        <div v-if="importError" class="alert error">
          ⚠️ 导入失败: {{ importError }}
        </div>
        <div v-if="successMsg" class="alert success">
          ✅ {{ successMsg }}
        </div>
      </div>

      <!-- 已保存订阅列表 -->
      <div class="panel-card">
        <h2>📋 已保存的订阅 ({{ subscriptions.length }})</h2>

        <div v-if="subscriptions.length === 0" class="empty-tip">
          暂无本地订阅，请在左侧表单中粘贴订阅链接导入。
        </div>

        <div v-else class="sub-list">
          <div v-for="sub in subscriptions" :key="sub.id" class="sub-item">
            <div class="sub-info">
              <span class="sub-title">{{ sub.name }}</span>
              <span class="sub-meta">
                格式: {{ sub.format.toUpperCase() }} · 节点数: {{ sub.node_count ?? 0 }}
              </span>
              <span class="sub-url-preview">{{ sub.url }}</span>
            </div>
            <button class="btn-danger" @click="handleRemove(sub.id)">删除</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-view {
  padding: var(--space-6);
  height: 100%;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.settings-header h1 {
  font-size: var(--text-xl);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
}

.settings-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-6);
}

@media (max-width: 800px) {
  .settings-grid {
    grid-template-columns: 1fr;
  }
}

.panel-card {
  background: var(--layer-1);
  backdrop-filter: var(--blur-panel);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-lg);
  padding: var(--space-6);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.panel-card h2 {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.panel-desc {
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.import-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.field-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.field-group label {
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.field-group input {
  padding: var(--space-2) var(--space-3);
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  font-size: var(--text-sm);
  outline: none;
  transition: border-color var(--duration-fast);
}

.field-group input:focus {
  border-color: var(--accent-blue);
}

.btn-primary {
  padding: var(--space-3);
  background: var(--accent-blue);
  color: #fff;
  border: none;
  border-radius: var(--radius-md);
  font-weight: var(--weight-medium);
  cursor: pointer;
  transition: opacity var(--duration-fast);
}

.btn-primary:hover:not(:disabled) {
  opacity: 0.9;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.alert {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
}

.alert.error {
  background: rgba(239, 68, 68, 0.15);
  border: 1px solid var(--accent-red);
  color: var(--accent-red);
}

.alert.success {
  background: rgba(16, 185, 129, 0.15);
  border: 1px solid var(--accent-green);
  color: var(--accent-green);
}

.empty-tip {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-align: center;
  padding: var(--space-6);
}

.sub-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.sub-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3);
  background: var(--layer-2);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
}

.sub-info {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.sub-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  color: var(--text-primary);
}

.sub-meta {
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.sub-url-preview {
  font-size: 11px;
  color: var(--text-tertiary);
  max-width: 260px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.btn-danger {
  padding: var(--space-1) var(--space-3);
  background: transparent;
  border: 1px solid var(--accent-red);
  color: var(--accent-red);
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  cursor: pointer;
}

.btn-danger:hover {
  background: rgba(239, 68, 68, 0.15);
}
</style>
