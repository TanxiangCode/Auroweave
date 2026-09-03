/**
 * 带超时控制的 Tauri IPC 客户端封装
 * 作者: TanXiang
 */
import { invoke } from "@tauri-apps/api/core";
import { IPC_TIMEOUT_MS } from "@/constants";

export { IPC_TIMEOUT_MS };

/** 超时 Error 的标记字段：调用方可通过 `err.isIpcTimeout === true` 精确区分超时与业务失败 */
export interface IpcTimeoutError extends Error {
  name: "TimeoutError";
  isIpcTimeout: true;
  cmd: string;
  timeoutMs: number;
}

/**
 * 执行带超时处理的 Tauri Invoke 命令
 * @param cmd 命令名称
 * @param args 命令参数
 * @param timeoutMs 超时时间(毫秒)，长任务（订阅拉取、内核升级等）应显式传入更长值
 */
export async function invokeWithTimeout<T>(
  cmd: string,
  args?: Record<string, unknown>,
  timeoutMs: number = IPC_TIMEOUT_MS
): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const timeoutPromise = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      // 超时仅意味着前端不再等待：后端命令可能仍在执行。
      // 打上 TimeoutError 标记，供调用方决定是否需要后端补偿（如取消/重试），
      // 而非笼统地当作业务失败处理
      const err = new Error(
        `IPC 命令 [${cmd}] 执行超时 (${timeoutMs}ms)`
      ) as IpcTimeoutError;
      err.name = "TimeoutError";
      err.isIpcTimeout = true;
      err.cmd = cmd;
      err.timeoutMs = timeoutMs;
      reject(err);
    }, timeoutMs);
  });

  try {
    const result = await Promise.race([
      invoke<T>(cmd, args),
      timeoutPromise,
    ]);
    return result;
  } finally {
    clearTimeout(timer!);
  }
}
