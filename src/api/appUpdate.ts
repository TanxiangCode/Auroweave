/**
 * 软件自更新（Auroweave 本体）—— 对接 tauri-plugin-updater / plugin-process
 *
 * 与内核升级（sing-box）刻意分开：内核升级走自定义 Rust 命令 + 事件流
 * （见 `singbox_update.rs`），而本体更新是 Tauri 官方 updater 的能力
 * （版本比对 + 签名校验 + 平台安装），这里只做薄薄一层封装，
 * 组件里不要直接 import 插件。
 */
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import type { ApiResponse, AppUpdateInfo } from "@/types";

/** 读取当前应用版本（来自 tauri.conf.json 的 version → 实际指向 package.json） */
export async function getAppVersion(): Promise<ApiResponse<string>> {
  try {
    return { success: true, data: await getVersion() };
  } catch (e) {
    return { success: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/** 检查结果：展示信息 + 插件 Update 句柄（后者用于"所见即所装"地下载） */
export interface AppUpdateCheck {
  info: AppUpdateInfo;
  update: Update;
}

/**
 * 检查是否有新版本。
 *
 * `check()` 拉取 endpoints 指向的 latest.json 并做**签名与版本比对**，
 * 返回 null 表示已是最新。抛错统一归一为 ApiResponse 失败态，
 * 避免调用方到处 try/catch。
 *
 * 返回值同时带展示用 `info` 与插件 `update` 句柄：调用方必须原样保存该句柄，
 * 供 `installAppUpdate` 使用——重新 check 会导致用户看到的版本与实际安装的不一致。
 */
export async function checkAppUpdate(): Promise<ApiResponse<AppUpdateCheck | null>> {
  try {
    const update = await check();
    if (!update) {
      return { success: true, data: null };
    }
    return {
      success: true,
      data: {
        update,
        info: {
          currentVersion: update.currentVersion,
          version: update.version,
          date: update.date ?? null,
          body: update.body ?? null,
        },
      },
    };
  } catch (e) {
    return { success: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/**
 * 下载并安装**指定的**更新对象。
 *
 * 为什么接收 update 而不在内部重新 `check()`：
 * 插件的 `Update` 持有端点返回的 `url` + `signature`，是一次检查结果的快照。
 * 若在这里重新 check，用户在确认弹窗上看到的版本号（来自上一次检查）可能与
 * 实际下载的不一致——发布方中途推了新版本时会出现"确认 0.8.0 却装上 0.8.1"，
 * 极端情况下 `check()` 还会返回 null 直接失败。传入既有对象保证"所见即所装"。
 *
 * 进度回调语义：`downloaded` / `total` 均为字节数。GitHub 走 chunked 传输时
 * `contentLength` 可能为 0，此时 `total === 0` —— 调用方必须据此隐藏百分比，
 * 否则会出现"已下 20MB 但进度显示 0%"的误导（内核升级踩过同一个坑）。
 *
 * 安装阶段由插件内部完成（Windows 上会自动退出安装），回调不再有事件，
 * 调用方应转为不确定态提示。
 */
export async function installAppUpdate(
  update: Update,
  onProgress: (p: { downloaded: number; total: number }) => void
): Promise<ApiResponse<null>> {
  try {
    let downloaded = 0;
    let total = 0;
    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength ?? 0;
          onProgress({ downloaded: 0, total });
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          onProgress({ downloaded, total });
          break;
        case "Finished":
          onProgress({ downloaded: total || downloaded, total: total || downloaded });
          break;
      }
    });
    return { success: true };
  } catch (e) {
    return { success: false, error: e instanceof Error ? e.message : String(e) };
  }
}

/**
 * 重启应用以使已安装的更新生效。
 *
 * macOS/Linux 上安装已落盘但进程仍是旧版本，必须换新进程才生效。
 * 注意：这是**不可撤销**操作——调用前应让用户明确确认。
 */
export async function relaunchApp(): Promise<ApiResponse<null>> {
  try {
    await relaunch();
    return { success: true, data: null };
  } catch (e) {
    return { success: false, error: e instanceof Error ? e.message : String(e) };
  }
}
