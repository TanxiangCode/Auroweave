/**
 * 连接审计与流控 Hook
 * 作者: TanXiang
 *
 * 职责：WebSocket 订阅、精确 ID 状态跟踪、语义化转换、历史队列归档与流控
 */
import { ref, computed, onActivated, onDeactivated } from "vue";
import { useConnectionStore } from "@/stores/connection.store";
import { subscribeConnections } from "@/api/clash-ws";
import { closeConnection, closeAllConnections } from "@/api/ipc/proxy";
import {
  translateConnection,
  type SemanticAuditRecord,
} from "@/utils/semantic-translator";
import type { Connection } from "@/types";
import { useToast } from "@/composables/useToast";

export function useConnectionAudit() {
  const connectionStore = useConnectionStore();
  const toast = useToast();

  const isPaused = ref(false);
  const activeRecords = ref<SemanticAuditRecord[]>([]);
  const historyRecords = ref<SemanticAuditRecord[]>([]);

  // 记录已知处理过的 ID 集合，避免重复计入统计
  const seenConnIds = new Set<string>();

  let unsub: (() => void) | null = null;

  onActivated(() => {
    if (unsub) return;
    unsub = subscribeConnections((payload) => {
      if (isPaused.value) return;

      const rawList: Connection[] = payload.connections || [];
      const currentIdSet = new Set<string>();
      const currentActive: SemanticAuditRecord[] = [];

      for (const conn of rawList) {
        currentIdSet.add(conn.id);
        const record = translateConnection(conn);
        currentActive.push(record);

        // 首次发现的新连接，计入统计与历史流
        if (!seenConnIds.has(conn.id)) {
          seenConnIds.add(conn.id);
          historyRecords.value.unshift(record);

          if (record.type === "proxied") connectionStore.stats.today_proxied++;
          else if (record.type === "direct") connectionStore.stats.today_direct++;
          else if (record.type === "blocked") connectionStore.stats.today_blocked++;
        }
      }

      activeRecords.value = currentActive;

      // 限制历史流容量 (200 条)
      if (historyRecords.value.length > 200) {
        historyRecords.value = historyRecords.value.slice(0, 200);
      }

      // 定期清理已过期的 seenConnIds，防止内存无限累积
      if (seenConnIds.size > 2000) {
        seenConnIds.clear();
        for (const c of rawList) seenConnIds.add(c.id);
      }
    });
  });

  onDeactivated(() => {
    if (unsub) {
      unsub();
      unsub = null;
    }
  });

  /** 关闭指定连接 */
  async function handleCloseConnection(id: string) {
    const res = await closeConnection(id);
    if (res.success) {
      activeRecords.value = activeRecords.value.filter((r) => r.id !== id);
      toast.success("连接已关闭", `成功终止连接 [${id}]`);
    } else {
      toast.error("关闭连接失败", res.error || "未知异常");
    }
  }

  /** 一键断开全部连接 */
  async function handleCloseAllConnections() {
    const res = await closeAllConnections();
    if (res.success) {
      activeRecords.value = [];
      toast.success("全部连接已切断", "当前所有活跃网络连接已终止");
    } else {
      toast.error("切断连接失败", res.error || "未知异常");
    }
  }

  /** 清空历史记录 */
  function clearHistory() {
    historyRecords.value = [];
    // 同步清空已见连接 ID 集合：否则清除后同 ID 连接的后续新会话会被当作旧连接漏记
    seenConnIds.clear();
    toast.info("历史记录已清空", "连接审计队列已重置");
  }

  return {
    isPaused,
    activeRecords,
    historyRecords,
    allRecords: computed(() => {
      // 优先展示活跃连接，其次展示历史连接
      const combined = [...activeRecords.value];
      const activeIds = new Set(activeRecords.value.map((r) => r.id));
      for (const h of historyRecords.value) {
        if (!activeIds.has(h.id)) {
          combined.push(h);
        }
      }
      return combined;
    }),
    handleCloseConnection,
    handleCloseAllConnections,
    clearHistory,
  };
}
