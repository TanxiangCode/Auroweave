import type { ApiResponse } from "@/types";
import { invokeWithTimeout } from "./client";

/** 配置编辑器加载结果（plan-Q） */
export interface ConfigEditorLoadResult {
  content: string;
  size: number;
  path: string;
}

/** 编辑器保存链结果（与后端 EditSaveResult 的 serde tag 形态对应） */
export type ConfigEditorSaveResult =
  | { status: "syntax_blocked"; line: number; column: number; message: string }
  | { status: "check_blocked"; location: string; message: string; raw_output: string }
  | { status: "saved"; size: number };

/** 导出当前内核的 JSON Schema（plan-Q Q1，写入数据目录 config/schema.json） */
export async function exportConfigSchema(): Promise<ApiResponse<string>> {
  return invokeWithTimeout<ApiResponse<string>>("config_export_schema", {}, 30000);
}

/** 加载 config.json 原文供编辑器展示 */
export async function loadConfigEditor(): Promise<ApiResponse<ConfigEditorLoadResult>> {
  return invokeWithTimeout<ApiResponse<ConfigEditorLoadResult>>("config_editor_load");
}

/** 校验并安全写回 config.json（L1 语法 → L3 check → 备份 + 原子写） */
export async function saveConfigEditor(
  content: string
): Promise<ApiResponse<ConfigEditorSaveResult>> {
  // check 子进程在大配置下可达数秒，放宽到 60s
  return invokeWithTimeout<ApiResponse<ConfigEditorSaveResult>>(
    "config_editor_save",
    { content },
    60000
  );
}

/** 重启内核使编辑后的 config.json 生效（保存不自动重启，由此命令确认触发） */
export async function restartCoreForEditor(): Promise<ApiResponse<void>> {
  return invokeWithTimeout<ApiResponse<void>>(
    "config_editor_restart_core",
    {},
    60000
  );
}
