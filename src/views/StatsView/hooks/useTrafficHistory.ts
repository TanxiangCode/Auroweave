/**
 * 历史流量数据获取 Hook
 * 作者: TanXiang
 *
 * 职责：按维度（日/月/年）获取分时流量历史数据
 */
import { ref, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

/** 柱状图数据点 */
export interface ChartPoint {
  label: string;
  bytes: number;
  heightPercent: number;
}

/**
 * 历史流量数据获取 Hook
 */
export function useTrafficHistory() {
  /** 时间维度 */
  const timeDimension = ref<"day" | "month" | "year">("day");
  /** 柱状图数据 */
  const chartData = ref<ChartPoint[]>([]);

  /** 获取流量历史 */
  async function fetchTrafficHistory() {
    try {
      const res: any = await invoke("get_traffic_history", { dimension: timeDimension.value });
      if (res.success && res.data) {
        const data = res.data;
        const totalBytesArr = data.map((d: any) => d.download_bytes + d.upload_bytes);
        const maxBytes = Math.max(...totalBytesArr, 1);

        chartData.value = data.map((d: any) => {
          const bytes = d.download_bytes + d.upload_bytes;
          return {
            label: d.label,
            bytes,
            heightPercent: (bytes / maxBytes) * 75,
          };
        });
      }
    } catch (e) {
      console.error("获取流量历史失败", e);
    }
  }

  // 维度切换时自动重新获取
  watch(timeDimension, fetchTrafficHistory);

  return {
    timeDimension,
    chartData,
    fetchTrafficHistory,
  };
}
