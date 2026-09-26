import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ApiResponse,
  AppSettings,
  SingboxUpdateInfo,
  SingboxUpdateProgress,
  SingboxVersionInfo,
} from "@/types";
import { invokeWithTimeout } from "./client";

/** 内核升级进度事件名（与后端 CORE_UPDATE_PROGRESS_EVENT 保持一致） */
export const CORE_UPGRADE_PROGRESS_EVENT = "core-update-progress";

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

/**
 * 在线升级 Sing-box 内核
 *
 * 后端在下载/解压/停核/替换/重启各阶段持续 emit `core-update-progress`，
 * 因此这里的 invoke 只是一次"启动确认"：真正的进度请读事件流或
 * `getCoreUpgradeStatus()`，不要把本 Promise 当作进度来源。
 */
export async function upgradeSingbox(downloadUrl: string): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>("core_upgrade_singbox", { downloadUrl }, 30000);
}

/** 读取当前已安装的 sing-box 内核版本与提权状态 */
export async function getSingboxVersion(): Promise<ApiResponse<SingboxVersionInfo>> {
  return invokeWithTimeout<ApiResponse<SingboxVersionInfo>>("proxy_get_singbox_version", {}, 10000);
}

/**
 * 回查内核升级当前进度
 *
 * 后端是进度真值源：前端切换页面/tab 导致组件卸载重建、甚至整页重载后，
 * 用它把按钮上的进度复原，避免"进度归零 → 用户重复触发内核替换"。
 */
export async function getCoreUpgradeStatus(): Promise<ApiResponse<SingboxUpdateProgress>> {
  return invokeWithTimeout<ApiResponse<SingboxUpdateProgress>>("core_upgrade_status", {}, 10000);
}

/** 监听内核升级实时进度事件 */
export async function listenCoreUpgradeProgress(
  callback: (payload: SingboxUpdateProgress) => void
): Promise<UnlistenFn> {
  return await listen<SingboxUpdateProgress>(CORE_UPGRADE_PROGRESS_EVENT, (event) => {
    callback(event.payload);
  });
}


