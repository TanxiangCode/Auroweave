/**
 * Pinia Store — 订阅管理
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { Subscription } from "@/types";
import {
  getSubscriptions,
  importSubscription,
  importContentSubscription,
  updateSubscriptionMeta,
  deleteSubscription,
  deleteAllSubscriptions,
  refreshSubscription,
  activateSubscription,
} from "@/api/ipc/subscription";
import { useToast } from "@/composables/useToast";
import { useProxyStore } from "@/stores/proxy.store";

export const useSubscriptionStore = defineStore("subscription", () => {
  const toast = useToast();
  // ---- 状态 ----
  const subscriptions = ref<Subscription[]>([]);
  const importing = ref(false);
  const importError = ref<string | null>(null);

  // ---- 计算属性 ----
  /** 当前活跃的订阅 */
  const activeSubscription = computed(() => {
    return subscriptions.value.find((s) => s.is_active) || null;
  });

  /** 检查 URL 是否已存在 */
  function hasUrl(url: string): boolean {
    return subscriptions.value.some((s) => s.url === url);
  }

  // ---- 动作 ----
  async function fetchAll() {
    const res = await getSubscriptions();
    if (res.success && res.data) {
      subscriptions.value = res.data;
    }
  }

  async function importSub(name: string, url: string, autoGroup = true) {
    importing.value = true;
    importError.value = null;
    try {
      const res = await importSubscription(name, url, autoGroup);
      if (res.success && res.data) {
        // 导入成功后重新从后端拉取全量列表，确保数据一致
        await fetchAll();
        try {
          await useProxyStore().fetchGroups();
        } catch (e) {
          console.warn("联动刷新代理节点列表失败:", e);
        }
      } else {
        importError.value = res.error ?? "导入失败";
      }
      return res;
    } finally {
      importing.value = false;
    }
  }

  async function importContentSub(
    name: string,
    content: string,
    sourceType: string,
    filePath?: string,
    autoGroup = true
  ) {
    importing.value = true;
    importError.value = null;
    try {
      const res = await importContentSubscription(name, content, sourceType, filePath, autoGroup);
      if (res.success && res.data) {
        await fetchAll();
        try {
          await useProxyStore().fetchGroups();
        } catch (e) {
          console.warn("联动刷新代理节点列表失败:", e);
        }
      } else {
        importError.value = res.error ?? "解析导入失败";
      }
      return res;
    } finally {
      importing.value = false;
    }
  }

  async function updateSubMeta(
    id: string,
    meta: {
      name?: string;
      url?: string;
      userAgent?: string;
      autoUpdateIntervalHours?: number;
      filterRule?: import("@/types").SubscriptionFilterRule;
    }
  ) {
    const res = await updateSubscriptionMeta(id, meta);
    if (res.success && res.data) {
      const idx = subscriptions.value.findIndex((s) => s.id === id);
      if (idx !== -1) subscriptions.value[idx] = res.data;
    }
    return res;
  }

  async function removeSub(id: string) {
    const res = await deleteSubscription(id);
    if (res.success) {
      subscriptions.value = subscriptions.value.filter((s) => s.id !== id);
    }
    return res;
  }

  async function removeAllSubs() {
    const res = await deleteAllSubscriptions();
    if (res.success) {
      subscriptions.value = [];
      toast.success("订阅列表已清空");
    }
    return res;
  }

  async function refreshSub(id: string) {
    const res = await refreshSubscription(id);
    if (res.success && res.data) {
      const idx = subscriptions.value.findIndex((s) => s.id === id);
      if (idx !== -1) subscriptions.value[idx] = res.data;
      try {
        await useProxyStore().fetchGroups();
      } catch (e) {
        console.warn("联动刷新代理节点列表失败:", e);
      }
    }
    return res;
  }

  async function activateSub(id: string) {
    // 先记住切换前的状态：后端是多选 toggle（只翻转目标订阅），
    // 若按"单选"语义回写会把其他仍在聚合中的订阅全部置为 false，
    // 导致界面状态与后端不一致（表现为其他卡片都变回"加入聚合"）。
    const wasActive = subscriptions.value.find((s) => s.id === id)?.is_active ?? false;

    const res = await activateSubscription(id);
    if (res.success && res.data) {
      // 以后端返回的该订阅为准做本地翻转，其余订阅状态原样保留
      const nextActive = res.data.is_active ?? !wasActive;
      subscriptions.value = subscriptions.value.map((s) =>
        s.id === id ? { ...s, is_active: nextActive } : s
      );
      try {
        await useProxyStore().fetchGroups();
      } catch (e) {
        console.warn("联动刷新代理节点列表失败:", e);
      }
    }
    return res;
  }

  return {
    subscriptions,
    activeSubscription,
    importing,
    importError,
    fetchAll,
    importSub,
    importContentSub,
    updateSubMeta,
    removeSub,
    removeAllSubs,
    refreshSub,
    activateSub,
    hasUrl,
  };
});

