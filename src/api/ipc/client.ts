/**
 * 带超时控制的 Tauri IPC 客户端封装
 * 作者: TanXiang
 */
import { invoke } from "@tauri-apps/api/core";
import { IPC_TIMEOUT_MS } from "@/constants";

/**
 * 执行带超时处理的 Tauri Invoke 命令
 * @param cmd 命令名称
 * @param args 命令参数
 * @param timeoutMs 超时时间(毫秒)
 */
export async function invokeWithTimeout<T>(
  cmd: string,
  args?: Record<string, unknown>,
  timeoutMs: number = IPC_TIMEOUT_MS
): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const timeoutPromise = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      reject(new Error(`IPC 命令 [${cmd}] 执行超时 (${timeoutMs}ms)`));
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
