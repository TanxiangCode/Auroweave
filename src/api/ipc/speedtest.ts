/**
 * IPC 封装层 — 测速命令
 * 作者: TanXiang
 */
import { invoke } from "@tauri-apps/api/core";
import type { ApiResponse, SpeedTestTask, ThroughputResult } from "@/types";

/** 运行单节点延迟测速 */
export async function runLatencyTest(
  nodeTags: string[]
): Promise<ApiResponse<void>> {
  return invoke<ApiResponse<void>>("speedtest_run_latency", { nodeTags });
}

/** 运行单节点吞吐量测速 */
export async function runSingleSpeedTest(
  nodeTag: string
): Promise<ApiResponse<ThroughputResult>> {
  return invoke<ApiResponse<ThroughputResult>>("speedtest_run_single", {
    nodeTag,
  });
}

/** 批量测速（串行，需用户二次确认后调用） */
export async function runBatchSpeedTest(
  groupTag: string
): Promise<ApiResponse<void>> {
  return invoke<ApiResponse<void>>("speedtest_run_batch", { groupTag });
}

/** 取消正在进行的批量测速 */
export async function cancelBatchSpeedTest(): Promise<ApiResponse<void>> {
  return invoke<ApiResponse<void>>("speedtest_cancel_batch");
}

/** 获取测速任务状态列表 */
export async function getSpeedTestResults(): Promise<
  ApiResponse<SpeedTestTask[]>
> {
  return invoke<ApiResponse<SpeedTestTask[]>>("speedtest_get_results");
}
