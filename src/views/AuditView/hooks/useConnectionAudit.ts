/**
 * 连接审计订阅 Hook
 * 作者: TanXiang
 *
 * 职责：WebSocket 订阅连接数据、语义化翻译、记录管理
 */
import { ref, onActivated, onDeactivated } from "vue";
import { useConnectionStore } from "@/stores/connection.store";
import { subscribeConnections } from "@/api/clash-ws";
import {
  translateConnection,
  type SemanticAuditRecord,
} from "@/utils/semantic-translator";

/**
 * 连接审计订阅 Hook
 */
export function useConnectionAudit() {
  const connectionStore = useConnectionStore();

  const isPaused = ref(false);
  const auditRecords = ref<SemanticAuditRecord[]>([]);

  let unsub: (() => void) | null = null;

  onActivated(() => {
    if (unsub) return;
    unsub = subscribeConnections((payload) => {
      if (isPaused.value) return;

      if (payload.connections && payload.connections.length > 0) {
        for (const conn of payload.connections.slice(0, 5)) {
          const domain = conn.destination || "未知主机";
          const outbound = conn.outbound || "direct";
          const ruleMatched = conn.rule || "default";

          const item = translateConnection(domain, outbound, ruleMatched);
          const record: SemanticAuditRecord = {
            ...item,
            id: Math.random().toString(36).substring(2, 9),
            timestamp: new Date().toLocaleTimeString("zh-CN", { hour12: false }),
          };

          auditRecords.value.unshift(record);
          if (record.type === "proxied") connectionStore.stats.today_proxied++;
          else if (record.type === "direct") connectionStore.stats.today_direct++;
          else if (record.type === "blocked") connectionStore.stats.today_blocked++;
        }

        if (auditRecords.value.length > 200) {
          auditRecords.value = auditRecords.value.slice(0, 200);
        }
      }
    });
  });

  onDeactivated(() => {
    if (unsub) {
      unsub();
      unsub = null;
    }
  });

  return {
    isPaused,
    auditRecords,
  };
}
