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
  deleteSubscription,
  deleteAllSubscriptions,
  refreshSubscription,
  activateSubscription,
} from "@/api/ipc/subscription";
import { useToast } from "@/composables/useToast";

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
    const res = await importSubscription(name, url, autoGroup);
    if (res.success && res.data) {
      // 导入成功后重新从后端拉取全量列表，确保数据一致
      await fetchAll();
    } else {
      importError.value = res.error ?? "导入失败";
    }
    importing.value = false;
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
    }
    return res;
  }

  async function activateSub(id: string) {
    const res = await activateSubscription(id);
    if (res.success && res.data) {
      // 更新本地列表的 is_active 状态
      subscriptions.value = subscriptions.value.map((s) => ({
        ...s,
        is_active: s.id === id,
      }));
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
    removeSub,
    removeAllSubs,
    refreshSub,
    activateSub,
    hasUrl,
  };
});
