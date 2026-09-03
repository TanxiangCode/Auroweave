/**
 * 系统服务管理 Hook
 * 作者: TanXiang
 *
 * 职责：服务状态轮询、安装/卸载/启停、运行模式切换、日志获取
 */
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import {
  serviceQueryStatus,
  serviceInstall,
  serviceUninstall,
  serviceStart,
  serviceStop,
  serviceReadLog,
} from "@/api/ipc/settings";

/** 服务状态数据结构 */
interface ServiceStatus {
  installedVersion: string | null;
  lastKnownStatus: string;
  lastFallbackReason: string | null;
}

/**
 * 系统服务管理 Hook
 */
export function useSystemService() {
  const settingsStore = useSettingsStore();

  /** 服务状态 */
  const serviceStatus = ref<ServiceStatus>({
    installedVersion: null,
    lastKnownStatus: "not_installed",
    lastFallbackReason: null,
  });

  /** 操作进行中标志 */
  const operating = ref(false);
  /** 错误消息 */
  const errorMsg = ref("");
  /** UAC 安装确认弹窗 */
  const showConfirmModal = ref(false);
  /** 服务日志弹窗 */
  const showLogModal = ref(false);
  /** 服务日志内容 */
  const serviceLog = ref("");

  /** 轮询定时器 */
  let statusTimer: ReturnType<typeof setInterval> | null = null;

  /** 当前运行模式 */
  const runMode = computed(() => {
    return settingsStore.settings.core?.runMode || "local";
  });

  /** 格式化状态显示文本 */
  const statusText = computed(() => {
    switch (serviceStatus.value.lastKnownStatus) {
      case "running": return "运行中";
      case "stopped": return "已停止";
      case "not_installed": return "未安装";
      case "error": return "异常";
      default: return "检测中";
    }
  });

  onMounted(() => {
    // 初次进入面板先查一次状态供展示，持续轮询仅在本机 service 模式下开启
    refreshStatus();
    if (runMode.value === "service") {
      startPolling();
    }
  });

  onUnmounted(() => {
    stopPolling();
  });

  // 运行模式切换时启停轮询：local 模式下无系统服务可查，无需持续 IPC 轮询
  watch(runMode, (mode) => {
    if (mode === "service") {
      startPolling();
    } else {
      stopPolling();
    }
  });

  function startPolling() {
    if (statusTimer) return;
    statusTimer = setInterval(refreshStatus, 3000);
  }

  function stopPolling() {
    if (statusTimer) {
      clearInterval(statusTimer);
      statusTimer = null;
    }
  }

  /** 刷新服务状态 */
  async function refreshStatus() {
    try {
      const res = await serviceQueryStatus();
      if (res.success && res.data) {
        serviceStatus.value = res.data;
      }
    } catch (e) {
      console.error("轮询服务状态异常:", e);
    }
  }

  /** 切换运行模式 */
  async function selectMode(mode: "local" | "service") {
    if (operating.value) return;
    errorMsg.value = "";
    operating.value = true;

    setTimeout(async () => {
      try {
        if (mode === "local") {
          const patch = {
            core: {
              runMode: "local" as const,
              service: { ...settingsStore.settings.core.service, lastKnownStatus: "stopped" as const },
            },
          };
          settingsStore.settings.core.runMode = "local";
          await settingsStore.updateSettings(patch);
          await refreshStatus();
        } else {
          const res = await serviceQueryStatus();
          if (res.success && res.data) {
            serviceStatus.value = res.data;
            if (res.data.lastKnownStatus === "not_installed") {
              showConfirmModal.value = true;
            } else {
              const patch = {
                core: {
                  ...settingsStore.settings.core,
                  runMode: "service" as const,
                },
              };
              settingsStore.settings.core.runMode = "service";
              await settingsStore.updateSettings(patch);
            }
          }
        }
      } catch (e: any) {
        errorMsg.value = e.message || "切换运行模式失败";
        await settingsStore.fetchSettings();
      } finally {
        operating.value = false;
      }
    }, 50);
  }

  /** UAC 安装服务 */
  async function handleInstallService() {
    showConfirmModal.value = false;
    operating.value = true;
    errorMsg.value = "";
    try {
      const res = await serviceInstall();
      if (res.success) {
        await settingsStore.fetchSettings();
        const patch = {
          core: {
            ...settingsStore.settings.core,
            runMode: "service" as const,
          },
        };
        await settingsStore.updateSettings(patch);
        await refreshStatus();
      } else {
        errorMsg.value = res.error || "服务安装未成功，UAC 授权可能已被取消";
      }
    } catch (e: any) {
      errorMsg.value = e.message || "执行安装服务请求时发生错误";
    } finally {
      operating.value = false;
    }
  }

  /** 启动服务 */
  async function handleStart() {
    operating.value = true;
    errorMsg.value = "";
    try {
      const res = await serviceStart();
      if (res.success) {
        await refreshStatus();
      } else {
        errorMsg.value = res.error || "启动服务失败";
      }
    } catch (e: any) {
      errorMsg.value = e.message || "启动服务异常";
    } finally {
      operating.value = false;
    }
  }

  /** 停止服务 */
  async function handleStop() {
    operating.value = true;
    errorMsg.value = "";
    try {
      const res = await serviceStop();
      if (res.success) {
        await refreshStatus();
      } else {
        errorMsg.value = res.error || "停止服务失败";
      }
    } catch (e: any) {
      errorMsg.value = e.message || "停止服务异常";
    } finally {
      operating.value = false;
    }
  }

  /** 卸载服务 */
  async function handleUninstall() {
    if (!confirm("确定要卸载 Auroweave 系统服务吗？此操作需要管理员权限。")) return;
    operating.value = true;
    errorMsg.value = "";
    try {
      const res = await serviceUninstall();
      if (res.success) {
        await settingsStore.fetchSettings();
        await refreshStatus();
      } else {
        errorMsg.value = res.error || "卸载服务失败";
      }
    } catch (e: any) {
      errorMsg.value = e.message || "卸载服务异常";
    } finally {
      operating.value = false;
    }
  }

  /** 查看服务日志 */
  async function handleViewLog() {
    operating.value = true;
    try {
      const res = await serviceReadLog();
      if (res.success && res.data) {
        serviceLog.value = res.data;
        showLogModal.value = true;
      } else {
        errorMsg.value = res.error || "无法加载服务日志";
      }
    } catch (e: any) {
      errorMsg.value = e.message || "加载服务日志异常";
    } finally {
      operating.value = false;
    }
  }

  return {
    // 状态
    serviceStatus,
    operating,
    errorMsg,
    showConfirmModal,
    showLogModal,
    serviceLog,
    runMode,
    statusText,
    // 方法
    refreshStatus,
    selectMode,
    handleInstallService,
    handleStart,
    handleStop,
    handleUninstall,
    handleViewLog,
  };
}
