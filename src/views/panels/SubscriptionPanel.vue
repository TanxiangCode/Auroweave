<template>
  <div class="panel-container">
    <h2>📦 订阅管理</h2>
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
        <h3>🚀 导入新订阅</h3>
        <div class="input-form">
          <input v-model="subName" type="text" placeholder="订阅别名 (如: SKYLUMO加速器)" class="text-input" />
          <input v-model="subUrl" type="text" placeholder="订阅 URL (如: https://...)" class="text-input" />
          <button class="btn-import" :disabled="importing" @click="handleImport">
            {{ importing ? '正在导入解析中...' : '🚀 开始导入' }}
          </button>
        </div>
      </div>

      <!-- 已导入订阅列表 -->
      <div class="subscriptions-card glass-effect">
        <h3>📋 已导入订阅 ({{ subscriptions.length }})</h3>
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
                <span class="sub-format">{{ sub.format.toUpperCase() }}</span>
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
import { ref } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { useProxyStore } from "@/stores/proxy.store";
import { importSubscription } from "@/api/ipc/subscription";
import { useToast } from "@/composables/useToast";

const settingsStore = useSettingsStore();
const subStore = useSubscriptionStore();
const proxyStore = useProxyStore();
const toast = useToast();

const subName = ref("SKYLUMO加速器");
const subUrl = ref("https://skylumo.com/api/v1/client/subscribe?token=REDACTED");
const importing = ref(false);
const operating = ref<string | null>(null);

const { subscriptions } = subStore;

async function save() {
  await settingsStore.updateSettings(settingsStore.settings);
  toast.success("订阅设置已保存");
}

async function handleImport() {
  if (!subName.value.trim() || !subUrl.value.trim()) {
    toast.warning("请输入订阅别名与链接");
    return;
  }
  importing.value = true;
  toast.info("正在网络拉取并解析订阅...", subName.value);

  const res = await importSubscription(
    subName.value.trim(),
    subUrl.value.trim(),
    settingsStore.settings.auto_group_on_import
  );

  importing.value = false;
  if (res.success) {
    toast.success("订阅导入成功！", `解析出 ${res.data?.node_count || 0} 个节点并拉起服务`);
    subName.value = "";
    subUrl.value = "";
    await subStore.fetchAll();
  } else {
    toast.error("订阅导入失败", res.error);
  }
}

async function handleActivate(id: string) {
  operating.value = id;
  const res = await subStore.activateSub(id);
  if (res.success) {
    toast.success("订阅已切换", `当前使用: ${res.data?.name}`);
    // 切换订阅后清空代理数据缓存并重新拉取
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
  // 刷新后清空代理数据缓存并重新拉取
    proxyStore.clearCache();
    await proxyStore.fetchGroups();
  } else {
    toast.error("刷新失败", res.error);
  }
  operating.value = null;
}

async function handleDelete(id: string) {
  const sub = subscriptions.find(s => s.id === id);
  if (!sub) return;

  if (sub.is_active) {
    toast.warning("无法删除", "请先切换到其他订阅，然后再删除此订阅");
    return;
  }

  if (!confirm(`确定要删除订阅「${sub.name}」吗？此操作不可恢复。`)) {
    return;
  }

  const res = await subStore.removeSub(id);
  if (res.success) {
    toast.success("订阅已删除");
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
.panel-container { display: flex; flex-direction: column; gap: 16px; }
h2 { font-size: 18px; font-weight: 700; }
.setting-group { display: flex; flex-direction: column; gap: 14px; }
.setting-item { display: flex; justify-content: space-between; align-items: center; padding: 14px 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 12px; }
.item-label { display: flex; flex-direction: column; gap: 4px; font-size: 14px; font-weight: 600; }
.sub-label { font-size: 11px; color: rgba(255,255,255,0.4); font-weight: normal; }
.switch { width: 18px; height: 18px; cursor: pointer; }
.import-card { padding: 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 14px; display: flex; flex-direction: column; gap: 12px; }
.import-card h3 { font-size: 15px; font-weight: 600; }
.input-form { display: flex; flex-direction: column; gap: 10px; }
.text-input { padding: 8px 12px; background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.12); border-radius: 8px; color: #fff; font-size: 13px; outline: none; }
.btn-import { padding: 10px; background: linear-gradient(135deg, #00f2fe, #4facfe); border: none; border-radius: 8px; color: #000; font-weight: 700; cursor: pointer; }
.btn-import:disabled { opacity: 0.6; cursor: not-allowed; }

.subscriptions-card { padding: 16px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.08); border-radius: 14px; display: flex; flex-direction: column; gap: 12px; }
.subscriptions-card h3 { font-size: 15px; font-weight: 600; }
.empty-tip { color: rgba(255,255,255,0.5); font-size: 13px; text-align: center; padding: 12px; }

.subscription-list { display: flex; flex-direction: column; gap: 8px; }
.subscription-item { display: flex; justify-content: space-between; align-items: center; padding: 12px 14px; background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.1); border-radius: 10px; gap: 10px; }
.subscription-item.active { background: rgba(0, 242, 254, 0.08); border-color: rgba(0, 242, 254, 0.3); }

.sub-info { flex: 1; display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.sub-header { display: flex; align-items: center; gap: 8px; }
.sub-name { font-size: 14px; font-weight: 600; color: #fff; }
.active-badge { font-size: 10px; padding: 2px 6px; background: #00f2fe; color: #000; border-radius: 10px; font-weight: 700; }
.sub-meta { display: flex; gap: 12px; font-size: 11px; color: rgba(255,255,255,0.6); }
.sub-format { font-family: var(--font-mono, monospace); }
.sub-nodes { font-weight: 500; }

.sub-actions { display: flex; gap: 6px; flex-shrink: 0; }
.btn-action { padding: 5px 10px; background: rgba(255,255,255,0.08); border: 1px solid rgba(255,255,255,0.15); border-radius: 6px; color: rgba(255,255,255,0.85); font-size: 11px; font-weight: 600; cursor: pointer; transition: all 0.15s ease; }
.btn-action:hover { background: rgba(255,255,255,0.15); color: #fff; }
.btn-action:disabled { opacity: 0.5; cursor: not-allowed; }

.btn-switch { background: rgba(0, 242, 254, 0.12); border-color: rgba(0, 242, 254, 0.3); color: #00f2fe; }
.btn-switch:hover { background: rgba(0, 242, 254, 0.2); }
.btn-refresh:hover { color: #00f2fe; border-color: rgba(0, 242, 254, 0.4); }
.btn-delete:hover { color: #ff4757; border-color: rgba(255, 71, 87, 0.4); }
</style>