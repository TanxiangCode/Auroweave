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

  // 语义转换缓存：id -> { fingerprint, record }
  // /connections WS 每秒推全量快照，原实现每拍对每条活跃连接重跑 8 条正则
  // + 重建对象（300 连接 ≈ 2400 regex/s + 300 对象/s GC 压力）。
  // 缓存键为影响转换结果的字段指纹——连接元数据未变时直接复用 record 对象。
  // 流量计数字段（upload/download bytes/speed）不计入指纹：它们不影响
  // 语义转换结果，且计数字段变化频繁会导致缓存永远 miss。
  const translateCache = new Map<
    string,
    { fingerprint: string; record: SemanticAuditRecord }
  >();

  /** 计算连接的语义指纹（转换输入相关的字段子集） */
  function fingerprintOf(conn: Connection): string {
    return [
      conn.destination,
      conn.destinationIP ?? "",
      conn.port,
      conn.outbound,
      conn.rule ?? "",
      conn.rulePayload ?? "",
      conn.process ?? "",
      conn.processPath ?? "",
      conn.network ?? "",
      conn.type ?? "",
      (conn.chains ?? []).join(">"),
    ].join("|");
  }

  /** 带缓存的语义转换：指纹命中时浅拷贝复用，同步刷新流量计数字段 */
  function translateCached(conn: Connection): SemanticAuditRecord {
    const fp = fingerprintOf(conn);
    const cached = translateCache.get(conn.id);
    if (cached && cached.fingerprint === fp) {
      // 语义字段（进程/域名/出站等）命中复用，但流量计数每拍都在涨——
      // 直接复用缓存对象会让连接表的"累计传输"列冻结在首拍数值。
      // 浅拷贝 + 仅更新 4 个计数字段：保留正则复用收益，恢复实时性。
      return {
        ...cached.record,
        upload_bytes: conn.upload_bytes || 0,
        download_bytes: conn.download_bytes || 0,
        upload_speed: conn.upload_speed,
        download_speed: conn.download_speed,
      };
    }
    const record = translateConnection(conn);
    translateCache.set(conn.id, { fingerprint: fp, record });
    return record;
  }

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
        const record = translateCached(conn);
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

      // 缓存收敛：移除已关闭连接的条目（与 seenConnIds 同节奏防泄漏）
      if (translateCache.size > 2000) {
        for (const id of translateCache.keys()) {
          if (!currentIdSet.has(id)) translateCache.delete(id);
        }
      }

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
    translateCache.clear();
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
