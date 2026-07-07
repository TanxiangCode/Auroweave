/**
 * IPC 封装层 — 应用设置命令
 * 作者: TanXiang
 */
import type { ApiResponse, AppSettings } from "@/types";
import { invokeWithTimeout } from "./client";

/** 获取所有设置 */
export async function getSettings(): Promise<ApiResponse<AppSettings>> {
  return invokeWithTimeout<ApiResponse<AppSettings>>("settings_get_all");
}

/** 保存设置（部分更新） */
export async function saveSettings(
  patch: Partial<AppSettings>
): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("settings_save", { patch });
}

/** 注入终端代理环境变量（开发者工具箱） */
export async function injectTerminalProxy(
  host: string,
  port: number
): Promise<ApiResponse<{ commands: string[] }>> {
  return invokeWithTimeout<ApiResponse<{ commands: string[] }>>(
    "settings_inject_terminal_proxy",
    { host, port }
  );
}

/** 导出诊断日志 */
export async function exportDiagnosticLog(): Promise<ApiResponse<string>> {
  return invokeWithTimeout<ApiResponse<string>>("settings_export_diagnostic_log");
}
