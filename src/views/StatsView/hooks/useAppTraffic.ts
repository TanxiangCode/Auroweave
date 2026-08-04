/**
 * 应用流量排行数据获取 Hook
 * 作者: TanXiang
 *
 * 职责：获取应用程序流量消耗 Top 10 数据
 */
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** 应用流量数据项 */
export interface AppTrafficItem {
  process_name: string;
  download_bytes: number;
  upload_bytes: number;
}

/**
 * 应用流量排行数据获取 Hook
 */
export function useAppTraffic() {
  /** 应用流量排行列表 */
  const topApps = ref<AppTrafficItem[]>([]);

  /** 获取应用流量统计 */
  async function fetchAppTraffic() {
    try {
      const res: any = await invoke("get_app_traffic_stats");
      if (res.success && res.data) {
        topApps.value = res.data;
      }
    } catch (e) {
      console.error("获取应用流量失败", e);
    }
  }

  return {
    topApps,
    fetchAppTraffic,
  };
}
