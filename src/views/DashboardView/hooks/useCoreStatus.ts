/**
 * 内核运行状态轮询 Hook
 * 作者: TanXiang
 *
 * 职责：定时轮询 sing-box 内核运行状态，管理 KeepAlive 下的生命周期
 */
import { type Ref, onActivated, onDeactivated, onMounted } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSettingsStore } from "@/stores/settings.store";
import { invoke } from "@tauri-apps/api/core";
import { info as logInfo, error as logError } from "@tauri-apps/plugin-log";
import type { ApiResponse } from "@/types";

interface UseCoreStatusOptions {
  proxyActive: Ref<boolean>;
  operating: Ref<boolean>;
}

/**
 * 内核运行状态轮询 Hook
 *
 * @param options - 依赖注入：代理激活状态与操作标志
 */
export function useCoreStatus(options: UseCoreStatusOptions) {
  const { proxyActive, operating } = options;
  const proxyStore = useProxyStore();
  const settingsStore = useSettingsStore();

  let statusTimer: ReturnType<typeof setInterval> | null = null;

  /** 轮询内核运行状态 */
  async function checkRunningStatus() {
    if (operating.value) return; // 在变更过程中避免覆盖乐观状态
    try {
      const runningRes: ApiResponse<boolean> = await invoke("core_query_running");
      if (runningRes.success) {
        proxyActive.value = runningRes.data;
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
      logInfo(`[DashboardView] 激活同步核心运行状态: ${proxyActive.value ? "运行中" : "未运行"}`);
    } catch (e: unknown) {
      proxyActive.value = false;
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
