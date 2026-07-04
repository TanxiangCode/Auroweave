/**
 * IPC 客户端 API — 智能测速
 * 作者: TanXiang
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ApiResponse, ThroughputResult } from "@/types";

export interface BatchProgressPayload {
  current_index: number;
  total: number;
  current_node: string;
  result?: ThroughputResult;
}

/** 对节点组执行延迟测速 */
export async function runLatencyTest(
  groupTag: string,
  nodeTags: string[]
): Promise<ApiResponse<Record<string, number>>> {
  return await invoke("speedtest_run_latency", { groupTag, nodeTags });
}

/** 针对单个节点触发吞吐量测速 */
export async function runSingleThroughputTest(
  nodeTag: string
): Promise<ApiResponse<ThroughputResult>> {
  return await invoke("speedtest_run_single", { nodeTag });
}

/** 启动批量串行测速 */
export async function runBatchSpeedTest(
  groupTag: string,
  nodeTags: string[]
): Promise<ApiResponse<void>> {
  return await invoke("speedtest_run_batch", { groupTag, nodeTags });
}

/** 取消当前正在运行的批量测速 */
export async function cancelBatchSpeedTest(): Promise<ApiResponse<void>> {
  return await invoke("speedtest_cancel_batch");
}

/** 获取已缓存的测速结果映射 */
export async function getSpeedTestResults(): Promise<ApiResponse<Record<string, ThroughputResult>>> {
  return await invoke("speedtest_get_results");
}

/** 监听批量测速实时进度事件 */
export async function listenSpeedTestProgress(
  callback: (payload: BatchProgressPayload) => void
): Promise<UnlistenFn> {
  return await listen<BatchProgressPayload>("speedtest-progress", (event) => {
    callback(event.payload);
  });
}
