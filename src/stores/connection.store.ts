/**
 * Pinia Store — 实时连接与流量统计
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, computed, onUnmounted } from "vue";
import type { TrafficSnapshot, Connection, ConnectionStats } from "@/types";
import { subscribeTraffic, subscribeConnections } from "@/api/clash-ws";
import { ENERGY_EMA_ALPHA } from "@/constants";

export const useConnectionStore = defineStore("connection", () => {
  // ---- 流量状态 ----
  const rawDownloadSpeed = ref(0);  // 原始网速，bytes/s
  const rawUploadSpeed = ref(0);
  /** EMA 平滑后的网速，驱动能量核动画 */
  const smoothDownloadSpeed = ref(0);
  const smoothUploadSpeed = ref(0);
  const totalDownload = ref(0);
  const totalUpload = ref(0);
  const activeConnectionCount = ref(0);

  // ---- 连接列表 ----
  const connections = ref<Connection[]>([]);

  // ---- 统计（Audit 页数据源） ----
  const stats = ref<ConnectionStats>({
    today_proxied: 0,
    today_direct: 0,
    today_blocked: 0,
    date: new Date().toISOString().slice(0, 10),
  });

  // ---- 计算属性 ----
  const isConnected = computed(() => activeConnectionCount.value > 0);

  // ---- 流量订阅 ----
  const unsubTraffic = subscribeTraffic((snapshot: TrafficSnapshot) => {
    rawDownloadSpeed.value = snapshot.download_speed;
    rawUploadSpeed.value = snapshot.upload_speed;
    totalDownload.value = snapshot.total_download;
    totalUpload.value = snapshot.total_upload;
    activeConnectionCount.value = snapshot.active_connections;

    // EMA 平滑（驱动能量核动画，避免瞬时抖动）
    smoothDownloadSpeed.value =
      ENERGY_EMA_ALPHA * snapshot.download_speed +
      (1 - ENERGY_EMA_ALPHA) * smoothDownloadSpeed.value;
    smoothUploadSpeed.value =
      ENERGY_EMA_ALPHA * snapshot.upload_speed +
      (1 - ENERGY_EMA_ALPHA) * smoothUploadSpeed.value;
  });

  const unsubConnections = subscribeConnections((payload) => {
    connections.value = payload.connections;
  });

  // Store 销毁时取消订阅
  onUnmounted(() => {
    unsubTraffic();
    unsubConnections();
  });

  return {
    rawDownloadSpeed,
    rawUploadSpeed,
    smoothDownloadSpeed,
    smoothUploadSpeed,
    totalDownload,
    totalUpload,
    activeConnectionCount,
    connections,
    stats,
    isConnected,
  };
});
