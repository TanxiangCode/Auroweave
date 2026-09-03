/**
 * Pinia Store — 实时连接与流量统计
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { TrafficSnapshot, Connection, ConnectionStats } from "@/types";
import { subscribeTraffic, subscribeConnections } from "@/api/clash-ws";
import { ENERGY_EMA_ALPHA, TRAFFIC_PERSIST_INTERVAL_MS } from "@/constants";
import { formatSpeed, formatBytes } from "@/utils/format";

export interface SpeedHistoryPoint {
  time: string;
  download: number;
  upload: number;
}

/** localStorage 键名 */
const LS_KEY_TOTAL_DOWNLOAD = "auroweave_total_download";
const LS_KEY_TOTAL_UPLOAD = "auroweave_total_upload";
/** 统计日期（YYYY-MM-DD）持久化键，用于跨天重置 today_* 计数 */
const LS_KEY_STATS_DATE = "auroweave_stats_date";

/** 读取 localStorage 数值并做 NaN/负数兜底（损坏数据归零） */
function readPersistedNumber(key: string): number {
  const raw = localStorage.getItem(key);
  if (raw === null) return 0;
  return Number(raw) || 0;
}

/** 读取持久化的统计日期，异常时回退当天 */
function readPersistedDate(): string {
  const raw = localStorage.getItem(LS_KEY_STATS_DATE);
  return /^\d{4}-\d{2}-\d{2}$/.test(raw || "") ? (raw as string) : "";
}

export const useConnectionStore = defineStore("connection", () => {
  // ---- 实时流量速率 (Bytes/s) ----
  const rawDownloadSpeed = ref(0);
  const rawUploadSpeed = ref(0);

  /** EMA 平滑后的网速，驱动能量核与界面动效 */
  const smoothDownloadSpeed = ref(0);
  const smoothUploadSpeed = ref(0);

  /** 累计流量 (Bytes) — 支持 localStorage 固化存盘 */
  const totalDownload = ref(readPersistedNumber(LS_KEY_TOTAL_DOWNLOAD));
  const totalUpload = ref(readPersistedNumber(LS_KEY_TOTAL_UPLOAD));

  /** 当前活动连接数量 */
  const activeConnectionCount = ref(0);

  /** 历史网速数据点（最多保留 60 秒用于折线图展示） */
  const speedHistory = ref<SpeedHistoryPoint[]>([]);

  // ---- 活动连接列表 ----
  const connections = ref<Connection[]>([]);

  // ---- 统计信息 ----
  const stats = ref<ConnectionStats>({
    today_proxied: 0,
    today_direct: 0,
    today_blocked: 0,
    date: readPersistedDate() || new Date().toISOString().slice(0, 10),
  });

  // ---- 计算属性 ----
  const isConnected = computed(() => activeConnectionCount.value > 0 || smoothDownloadSpeed.value > 1024);

  // ---- 节流持久化累计流量 ----
  // traffic 事件每秒一次，同步写 localStorage 会造成高频磁盘 IO；
  // 改为每 TRAFFIC_PERSIST_INTERVAL_MS 落盘一次，页面隐藏时立即补写
  let lastPersistAt = 0;

  function persistTotals(): void {
    try {
      localStorage.setItem(LS_KEY_TOTAL_DOWNLOAD, String(totalDownload.value));
      localStorage.setItem(LS_KEY_TOTAL_UPLOAD, String(totalUpload.value));
      localStorage.setItem(LS_KEY_STATS_DATE, stats.value.date);
    } catch {
      // localStorage 不可用（隐私模式等）时静默降级为内存统计
    }
  }

  function persistTotalsThrottled(): void {
    const now = Date.now();
    if (now - lastPersistAt < TRAFFIC_PERSIST_INTERVAL_MS) return;
    lastPersistAt = now;
    persistTotals();
  }

  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", () => {
      // 页面隐藏前立即落盘，最大限度缩小崩溃时的统计误差
      if (document.hidden) persistTotals();
    });
  }

  // ---- 跨天重置"今日"统计 ----
  // useConnectionAudit（views 层）直接累加 stats.today_*，重置职责收敛在 store 内部，
  // 由每秒到达的 traffic 事件驱动检查，保证长驻后台也能跨天归零
  function ensureDailyReset(): void {
    const today = new Date().toISOString().slice(0, 10);
    if (stats.value.date !== today) {
      stats.value = {
        today_proxied: 0,
        today_direct: 0,
        today_blocked: 0,
        date: today,
      };
    }
  }

  // ---- WebSocket 订阅监听 ----
  subscribeTraffic((snapshot: TrafficSnapshot) => {
    ensureDailyReset();

    rawDownloadSpeed.value = snapshot.download_speed;
    rawUploadSpeed.value = snapshot.upload_speed;

    // 累计数据内存累加，节流落盘（不再每秒写 localStorage）
    totalDownload.value += snapshot.download_speed;
    totalUpload.value += snapshot.upload_speed;
    persistTotalsThrottled();

    // EMA 平滑算法 (α=0.15)
    smoothDownloadSpeed.value =
      ENERGY_EMA_ALPHA * snapshot.download_speed +
      (1 - ENERGY_EMA_ALPHA) * smoothDownloadSpeed.value;
    smoothUploadSpeed.value =
      ENERGY_EMA_ALPHA * snapshot.upload_speed +
      (1 - ENERGY_EMA_ALPHA) * smoothUploadSpeed.value;

    // 追加历史采样点 (限制 60 点)
    const timeStr = new Date().toLocaleTimeString("zh-CN", { hour12: false });
    speedHistory.value.push({
      time: timeStr,
      download: snapshot.download_speed,
      upload: snapshot.upload_speed,
    });
    if (speedHistory.value.length > 60) {
      speedHistory.value.shift();
    }
  });

  subscribeConnections((payload) => {
    connections.value = payload.connections || [];
    activeConnectionCount.value = connections.value.length;
  });

  // WebSocket 订阅已升级为常驻生命周期，解决页面切换后的流量漏记和图形凝固Bug

  return {
    rawDownloadSpeed,
    rawUploadSpeed,
    smoothDownloadSpeed,
    smoothUploadSpeed,
    totalDownload,
    totalUpload,
    activeConnectionCount,
    speedHistory,
    connections,
    stats,
    isConnected,
    formatSpeed,
    formatBytes,
    /** 供外部（如审计视图挂载时）主动触发跨天检查与重置 */
    ensureDailyReset,
  };
});
