/**
 * Pinia Store — 实时连接与流量统计
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed } from "vue";
import type { TrafficSnapshot, Connection, ConnectionStats } from "@/types";
import { subscribeTraffic, subscribeConnections } from "@/api/clash-ws";
import { ENERGY_EMA_ALPHA } from "@/constants";

export interface SpeedHistoryPoint {
  time: string;
  download: number;
  upload: number;
}

export const useConnectionStore = defineStore("connection", () => {
  // ---- 实时流量速率 (Bytes/s) ----
  const rawDownloadSpeed = ref(0);
  const rawUploadSpeed = ref(0);

  /** EMA 平滑后的网速，驱动能量核与界面动效 */
  const smoothDownloadSpeed = ref(0);
  const smoothUploadSpeed = ref(0);

  /** 累计流量 (Bytes) */
  const totalDownload = ref(0);
  const totalUpload = ref(0);

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
    date: new Date().toISOString().slice(0, 10),
  });

  // ---- 计算属性 ----
  const isConnected = computed(() => activeConnectionCount.value > 0 || smoothDownloadSpeed.value > 1024);

  // 辅助格式化网速
  const formatSpeed = (bytesPerSec: number): string => {
    if (bytesPerSec < 1024) return `${bytesPerSec.toFixed(0)} B/s`;
    if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`;
    if (bytesPerSec < 1024 * 1024 * 1024) return `${(bytesPerSec / (1024 * 1024)).toFixed(2)} MB/s`;
    return `${(bytesPerSec / (1024 * 1024 * 1024)).toFixed(2)} GB/s`;
  };

  const formatBytes = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  // ---- WebSocket 订阅监听 ----
  subscribeTraffic((snapshot: TrafficSnapshot) => {
    rawDownloadSpeed.value = snapshot.download_speed;
    rawUploadSpeed.value = snapshot.upload_speed;

    // 累计数据累加
    totalDownload.value += snapshot.download_speed;
    totalUpload.value += snapshot.upload_speed;

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
  };
});
