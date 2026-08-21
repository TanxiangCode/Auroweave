/**
 * 内核运行状态轮询 Hook
 * 作者: TanXiang
 *
 * 职责：定时轮询 sing-box 内核运行状态，管理 KeepAlive 下的生命周期
 * 当检测到内核从停止变为运行时，主动触发 WebSocket 重连以恢复实时数据流
 * 在应用启动时显示"启动中"中间状态，避免从灰色待机直接跳变为已连接
 */
import { type Ref, onActivated, onDeactivated, onMounted } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSettingsStore } from "@/stores/settings.store";
import { invoke } from "@tauri-apps/api/core";
import { info as logInfo, error as logError } from "@tauri-apps/plugin-log";
import { reconnectAll, setProxyActiveStatus } from "@/api/clash-ws";
import type { ApiResponse } from "@/types";


interface UseCoreStatusOptions {
  proxyActive: Ref<boolean>;
  coreStarting: Ref<boolean>;
  operating: Ref<boolean>;
}

/**
 * 内核运行状态轮询 Hook
 *
 * @param options - 依赖注入：代理激活状态、启动中标志、操作标志
 */
export function useCoreStatus(options: UseCoreStatusOptions) {
  const { proxyActive, coreStarting, operating } = options;
  const proxyStore = useProxyStore();
  const settingsStore = useSettingsStore();

  let statusTimer: ReturnType<typeof setInterval> | null = null;
  /** 记录上一次的内核运行状态，用于检测 0→1 变化并触发 WebSocket 重连 */
  let wasRunning = false;
  /** 应用启动后自动启动的防抖计数器，避免频繁的状态切换 */
  let startupPollCount = 0;
  /** 已检测到首次启动完成 */
  let startupCompleted = false;

  /** 轮询内核运行状态 */
  async function checkRunningStatus() {
    if (operating.value) return; // 在变更过程中避免覆盖乐观状态
    try {
      const runningRes: ApiResponse<boolean> = await invoke("core_query_running");
      if (runningRes.success) {
        const isRunning = runningRes.data ?? false;

        // 应用启动阶段：如果代理模式不是 direct，显示"正在启动"中间状态
        if (!startupCompleted && !isRunning) {
          startupPollCount++;

          // 首次轮询且配置不是 direct 模式时，说明后端正在自动拉起 sing-box
          if (startupPollCount === 1 && settingsStore.settings.proxy_mode !== "direct") {
            coreStarting.value = true;
            logInfo("[DashboardView] 应用启动中，内核正在拉起，显示启动中状态...");
            return;
          }

          // 超过约30秒（10次轮询 × 3秒）还未启动，认为启动失败
          if (startupPollCount > 10 && settingsStore.settings.proxy_mode !== "direct") {
            logInfo("[DashboardView] 启动超时，保持待机状态");
            startupCompleted = true;
            coreStarting.value = false;
          }
        }

        // 检测到内核启动完成
        if (isRunning && !wasRunning && !startupCompleted) {
          startupCompleted = true;
          coreStarting.value = false;
          logInfo("[DashboardView] 检测到内核已启动，触发 WebSocket 重连");
          reconnectAll();
        }

        proxyActive.value = isRunning;
        setProxyActiveStatus(isRunning);
        wasRunning = isRunning;
      }

    } catch (e: unknown) {
      console.error("轮询内核状态异常:", e);
    }
  }

  // 首次挂载：仅加载一次设置
  onMounted(async () => {
    await settingsStore.fetchSettings();
  });

  // 每次激活：同步状态、拉取分组、启动轮询
  onActivated(async () => {
    proxyStore.fetchGroups();

    try {
      await checkRunningStatus();
      const statusText = coreStarting.value
        ? "启动中"
        : proxyActive.value
          ? "运行中"
          : "未运行";
      logInfo(`[DashboardView] 激活同步核心运行状态: ${statusText}`);
    } catch (e: unknown) {
      proxyActive.value = false;
      coreStarting.value = false;
      logError(`[DashboardView] 获取核心运行状态异常: ${e instanceof Error ? e.message : e}`);
    }

    if (statusTimer) clearInterval(statusTimer);
    statusTimer = setInterval(checkRunningStatus, 3000);
  });

  // 离开页面时停止轮询
  onDeactivated(() => {
    if (statusTimer) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
  });

  return { checkRunningStatus };
}
