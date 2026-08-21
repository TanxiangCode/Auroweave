<template>
  <div class="panel-container">
    <h2>订阅管理</h2>
    <div class="setting-group">
      <div class="setting-item">
        <div class="item-label">
          <span>自动整理地区分组</span>
          <span class="sub-label">导入订阅时自动识别 HK/JP/US 等地区并生成 urltest 延迟优选组</span>
        </div>
        <input type="checkbox" v-model="settingsStore.settings.auto_group_on_import" class="switch" @change="save" />
      </div>

      <!-- 快速导入模组 -->
      <div class="import-card glass-effect">
        <h3>导入新订阅</h3>
        <div class="input-form">
          <input v-model="subName" type="text" placeholder="订阅别名 (如: SKYLUMO加速器)" class="text-input" />
          <input v-model="subUrl" type="text" placeholder="订阅 URL (如: https://...)" class="text-input" />
          <button class="btn-import" :disabled="importing" @click="handleImport">
            {{ importing ? '正在导入解析中...' : '开始导入' }}
          </button>
        </div>
      </div>

      <!-- 已导入订阅列表 -->
      <div class="subscriptions-card glass-effect">
        <h3>已导入订阅 ({{ subscriptions.length }})</h3>
        <div v-if="subscriptions.length === 0" class="empty-tip">
          尚未导入任何订阅，请在上方添加
        </div>
        <div v-else class="subscription-list">
          <div
            v-for="sub in subscriptions"
            :key="sub.id"
            class="subscription-item"
            :class="{ active: sub.is_active }"
          >
            <div class="sub-info">
              <div class="sub-header">
                <span class="sub-name">{{ sub.name }}</span>
                <span v-if="sub.is_active" class="active-badge">当前使用</span>
              </div>
              <div class="sub-meta">
                <span class="sub-format">{{ (sub.format || 'unknown').toUpperCase() }}</span>
                <span class="sub-nodes">{{ sub.node_count || 0 }} 节点</span>
                <span v-if="sub.last_updated" class="sub-time">
                  更新于 {{ formatTime(sub.last_updated) }}
                </span>
              </div>
            </div>
            <div class="sub-actions">
              <button
                v-if="!sub.is_active"
                class="btn-action btn-switch"
                :disabled="operating === sub.id"
                @click="handleActivate(sub.id)"
                title="切换到此订阅"
              >
                切换
              </button>
              <button
                class="btn-action btn-refresh"
                :disabled="operating === sub.id"
                @click="handleRefresh(sub.id)"
                title="刷新订阅"
              >
                刷新
              </button>
              <button
                class="btn-action btn-delete"
                :disabled="operating === sub.id"
                @click="handleDelete(sub.id)"
                title="删除订阅"
              >
                删除
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useSettingsStore } from "@/stores/settings.store";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { useProxyStore } from "@/stores/proxy.store";
import { useToast } from "@/composables/useToast";

const settingsStore = useSettingsStore();
const subStore = useSubscriptionStore();
const proxyStore = useProxyStore();
const toast = useToast();

const subName = ref("");
const subUrl = ref("");
const importing = ref(false);
const operating = ref<string | null>(null);

const { subscriptions } = storeToRefs(subStore);

onMounted(() => {
  subStore.fetchAll();
});

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("订阅设置已保存");
}

async function handleImport() {
  if (!subName.value.trim() || !subUrl.value.trim()) {
    toast.warning("请输入订阅别名与链接");
    return;
  }

  const urlTrimmed = subUrl.value.trim();

  const existing = subStore.subscriptions.find(s => s.url === urlTrimmed);
  if (existing) {
    const confirmMsg = `订阅「${existing.name}」已存在。\n\n是否要覆盖它并重新导入最新节点？`;
    if (!confirm(confirmMsg)) {
      return;
    }
  }

  importing.value = true;
  toast.info("正在网络拉取并解析订阅...", subName.value);

  const res = await subStore.importSub(
    subName.value.trim(),
    urlTrimmed,
    settingsStore.settings.auto_group_on_import
  );

  importing.value = false;
  if (res.success) {
    toast.success("订阅导入成功！", `解析出 ${res.data?.node_count || 0} 个节点并拉起服务`);
    subName.value = "";
    subUrl.value = "";
  } else {
    toast.error("订阅导入失败", res.error);
  }
}

async function handleActivate(id: string) {
  operating.value = id;
  const res = await subStore.activateSub(id);
  if (res.success) {
    toast.success("订阅已切换", `当前使用: ${res.data?.name}`);
    proxyStore.clearCache();
    await proxyStore.fetchGroups();
  } else {
    toast.error("切换失败", res.error);
  }
  operating.value = null;
}

  async function handleRefresh(id: string) {
    operating.value = id;
    const res = await subStore.refreshSub(id);
    if (res.success) {
      toast.success("订阅刷新成功", `解析出 ${res.data?.node_count || 0} 个节点`);
      proxyStore.clearCache();
      // 关键修复：等待 sing-box 完全就绪后再拉取代理列表，避免 ClashAPI 请求失败
      // 后端 build_and_apply_config 已等待 ClashAPI 就绪，但前端也需要短暂缓冲
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // 重试机制：最多尝试3次拉取代理列表
      let retryCount = 0;
      let groupsFetched = false;
      while (retryCount < 3 && !groupsFetched) {
        await proxyStore.fetchGroups();
        if (proxyStore.groups.length > 0) {
          groupsFetched = true;
          break;
        }
        retryCount++;
        if (retryCount < 3) {
          await new Promise(resolve => setTimeout(resolve, 1000));
        }
      }
      
      if (!groupsFetched) {
        toast.warning("代理列表暂时为空", "Sing-box 可能还在初始化，请稍后刷新");
      }
    } else {
      toast.error("刷新失败", res.error);
    }
    operating.value = null;
  }

  async function handleDelete(id: string) {
  const sub = subscriptions.value.find(s => s.id === id);
  if (!sub) return;

  if (!confirm(`确定要删除订阅「${sub.name}」吗？此操作不可恢复。`)) {
    return;
  }

  const res = await subStore.removeSub(id);
  if (res.success) {
    toast.success("订阅已删除");
    if (sub.is_active) {
      proxyStore.clearCache();
    }
  } else {
    toast.error("删除失败", res.error);
  }
}

function formatTime(ts?: number): string {
  if (!ts) return "";
  const d = new Date(ts);
  return d.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
}
</script>

<style scoped>
.switch { width: 18px; height: 18px; cursor: pointer; }

.import-card {
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.import-card h3 {
  font-size: var(--text-base);
  font-weight: var(--weight-semibold);
}

.input-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.text-input {
  padding: var(--space-2) var(--space-3);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  outline: none;
}

.btn-import {
  padding: var(--space-3);
  background: linear-gradient(135deg, var(--accent-cyan-vivid), var(--accent-blue));
  border: none;
  border-radius: var(--radius-sm);
  color: var(--layer-0);
  font-weight: var(--weight-bold);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-import:disabled { opacity: 0.6; cursor: not-allowed; }
.btn-import:hover:not(:disabled) { filter: brightness(1.1); }

.subscriptions-card {
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.subscriptions-card h3 {
  font-size: var(--text-base);
  font-weight: var(--weight-semibold);
}

.empty-tip {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  text-align: center;
  padding: var(--space-3);
}

.subscription-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.subscription-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-3) var(--space-4);
  background: var(--surface-raised);
  border: 1px solid var(--border-normal);
  border-radius: var(--radius-md);
  gap: var(--space-3);
}

.subscription-item.active {
  background: var(--accent-cyan-glow);
  border-color: var(--border-accent);
}

.sub-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.sub-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.sub-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.active-badge {
  font-size: var(--text-xs);
  padding: var(--space-1) var(--space-2);
  background: var(--accent-cyan-vivid);
  color: var(--layer-0);
  border-radius: var(--radius-full);
  font-weight: var(--weight-bold);
}

.sub-meta {
  display: flex;
  gap: var(--space-3);
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.sub-format { font-family: var(--font-mono); }
.sub-nodes { font-weight: var(--weight-medium); }

.sub-actions {
  display: flex;
  gap: var(--space-2);
  flex-shrink: 0;
}

.btn-action {
  padding: var(--space-1) var(--space-3);
  background: var(--surface-hover);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  cursor: pointer;
  transition: all var(--duration-fast);
}

.btn-action:hover { background: var(--border-strong); }
.btn-action:disabled { opacity: 0.5; cursor: not-allowed; }

.btn-switch {
  background: var(--accent-cyan-glow);
  border-color: var(--border-accent);
  color: var(--accent-cyan-vivid);
}

.btn-switch:hover { background: var(--accent-cyan-vivid); color: var(--layer-0); }

.btn-refresh:hover {
  color: var(--accent-cyan-vivid);
  border-color: var(--border-accent);
}

.btn-delete:hover {
  color: var(--accent-red);
  border-color: var(--accent-red);
}
</style>
