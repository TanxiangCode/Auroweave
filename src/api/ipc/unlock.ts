/**
 * IPC 客户端 API — AI 服务解锁检测
 * 作者: TanXiang
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ApiResponse, UnlockCheckResult, UnlockServiceId } from "@/types";
import { invokeWithTimeout } from "./client";

/** 单节点检测：3 服务 + ip-api 层，按 15s/请求放宽到 60s */
export async function runUnlockCheckSingle(
  nodeTag: string,
  services: UnlockServiceId[] = [],
  withIp = true
): Promise<ApiResponse<UnlockCheckResult>> {
  return await invokeWithTimeout(
    "unlock_check_single",
    { nodeTag, services, withIp },
    60000
  );
}

/** 启动批量解锁检测（fire-and-forget，进度经 unlock-check-progress 事件） */
export async function runUnlockCheckBatch(
  groupTag: string,
  nodeTags: string[],
  services: UnlockServiceId[] = [],
  withIp = true
): Promise<ApiResponse<void>> {
  return await invokeWithTimeout(
    "unlock_check_batch",
    { groupTag, nodeTags, services, withIp }
  );
}

/** 取消正在运行的批量解锁检测 */
export async function cancelUnlockCheckBatch(): Promise<ApiResponse<void>> {
  return await invokeWithTimeout("unlock_check_cancel");
}

/** 每节点最近一次解锁检测结果（启动/激活时回填） */
export async function getLatestUnlockResults(): Promise<
  ApiResponse<UnlockCheckResult[]>
> {
  return await invokeWithTimeout("unlock_check_get_latest", undefined, 10000);
}

export interface UnlockBatchProgressPayload {
  current_index: number;
  total: number;
  current_node: string;
  result?: UnlockCheckResult;
}

/** 监听批量解锁检测进度事件 */
export async function listenUnlockCheckProgress(
  callback: (payload: UnlockBatchProgressPayload) => void
): Promise<UnlistenFn> {
  return await listen<UnlockBatchProgressPayload>(
    "unlock-check-progress",
    (event) => {
      callback(event.payload);
    }
  );
}
