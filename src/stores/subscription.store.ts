/**
 * Pinia Store — 订阅管理
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref } from "vue";
import type { Subscription } from "@/types";
import {
  getSubscriptions,
  importSubscription,
  deleteSubscription,
  refreshSubscription,
} from "@/api/ipc/subscription";

export const useSubscriptionStore = defineStore("subscription", () => {
  // ---- 状态 ----
  const subscriptions = ref<Subscription[]>([]);
  const importing = ref(false);
  const importError = ref<string | null>(null);

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
      subscriptions.value.push(res.data);
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

  async function refreshSub(id: string) {
    const res = await refreshSubscription(id);
    if (res.success && res.data) {
      const idx = subscriptions.value.findIndex((s) => s.id === id);
      if (idx !== -1) subscriptions.value[idx] = res.data;
    }
    return res;
  }

  return {
    subscriptions,
    importing,
    importError,
    fetchAll,
    importSub,
    removeSub,
    refreshSub,
  };
});
