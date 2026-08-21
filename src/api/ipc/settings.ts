import type { ApiResponse, AppSettings, SingboxUpdateInfo } from "@/types";
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

/** 查询 Windows 服务状态 */
export async function serviceQueryStatus(): Promise<ApiResponse<AppSettings["core"]["service"]>> {
  return invokeWithTimeout<ApiResponse<AppSettings["core"]["service"]>>("service_query_status");
}

/** 提权安装 Windows 服务 */
export async function serviceInstall(): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("service_install", {}, 30000); // 提权需要较长超时
}

/** 提权卸载 Windows 服务 */
export async function serviceUninstall(): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("service_uninstall", {}, 30000);
}

/** 启动 Windows 服务 */
export async function serviceStart(): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("service_start");
}

/** 停止 Windows 服务 */
export async function serviceStop(): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("service_stop");
}

/** 读取 Windows 服务日志 */
export async function serviceReadLog(): Promise<ApiResponse<string>> {
  return invokeWithTimeout<ApiResponse<string>>("service_read_log");
}

/** 从备份还原 config.backup.json 并重启核心 */
export async function restoreConfigBackup(): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("settings_restore_backup");
}

/** 检查 Sing-box 内核版本更新 */
export async function checkSingboxUpdate(): Promise<ApiResponse<SingboxUpdateInfo>> {
  return invokeWithTimeout<ApiResponse<SingboxUpdateInfo>>("core_check_singbox_update", {}, 20000);
}

/** 在线升级 Sing-box 内核 */
export async function upgradeSingbox(downloadUrl: string): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("core_upgrade_singbox", { downloadUrl }, 180000);
}


