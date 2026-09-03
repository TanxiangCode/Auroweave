/**
 * 代理开关 Hook
 * 作者: TanXiang
 *
 * 职责：管理代理激活状态、乐观更新/回滚、分流模式切换
 */
import { ref } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSettingsStore } from "@/stores/settings.store";
import { useToast } from "@/composables/useToast";
import { invoke } from "@tauri-apps/api/core";
import { warn as logWarn } from "@tauri-apps/plugin-log";
import type { ApiResponse } from "@/types";

/**
 * 代理开关 Hook
 *
 * 拥有 proxyActive 和 operating 状态，供其他 Hook 共享
 */
export function useProxyToggle() {
  const proxyStore = useProxyStore();
  const settingsStore = useSettingsStore();
  const toast = useToast();

  /** 代理流量接管总开关 */
  const proxyActive = ref(false); // 初始设为 false，由 checkRunningStatus 决定实际状态
  /** 是否正在启动核心 */
  const coreStarting = ref(false);
  /** 操作进行中标志（防重入） */
  const operating = ref(false);
  /**
   * 乐观更新保护窗口截止时间戳（毫秒）
   * 在 toggleProxy 乐观更新后、内核确认前，轮询不得覆盖 proxyActive
   */
  const recentToggleUntil = ref(0);

  /**
   * 统一的失败回滚：恢复 tun_enabled / proxy_mode / proxyStore.proxyMode
   * @param targetActive 回滚后的 proxyActive 目标值（toggle 失败时为切换前的值）
   */
  function rollback(targetActive: boolean, tunEnabled: boolean, proxyMode: "global" | "rule" | "direct") {
    proxyActive.value = targetActive;
    settingsStore.settings.tun_enabled = tunEnabled;
    settingsStore.settings.proxy_mode = proxyMode;
    proxyStore.$patch({ proxyMode });
  }

  /** 一键接管网络 / 完全注销释放 Windows 代理 */
  function toggleProxy() {
    if (operating.value) return;
    operating.value = true;

    const prevActive = proxyActive.value;
    const prevTun = settingsStore.settings.tun_enabled;
    const prevMode = settingsStore.settings.proxy_mode as "global" | "rule" | "direct";
    const nextActive = !prevActive;
    // 乐观更新
    proxyActive.value = nextActive;
    // 开启乐观更新保护窗口：10 秒内轮询不覆盖 proxyActive
    recentToggleUntil.value = Date.now() + 10000;

    setTimeout(async () => {
      try {
        if (nextActive) {
          if (settingsStore.settings.tun_enabled) {
            // TUN 模式：通过 proxyMode 参数一次性设置 tun_enabled + proxy_mode，
            // 避免先 tun_set_enabled 再 changeProxyMode 导致的第二次进程重启（macOS 第二次密码框）
            const proxyMode = settingsStore.settings.proxy_mode === "direct" ? "rule" : null;
            const res: ApiResponse = await invoke("tun_set_enabled", { enabled: true, proxyMode });
            if (res.success) {
              settingsStore.settings.tun_enabled = true;
              if (proxyMode) {
                settingsStore.settings.proxy_mode = proxyMode;
                proxyStore.$patch({ proxyMode });
              }
              // 不再调用 changeProxyMode — tun_set_enabled 已用正确的 proxy_mode 启动 sing-box
            } else {
              const isMac = navigator.userAgent.toLowerCase().includes("mac");
              if (isMac) {
                // macOS: TUN 提权通过 osascript 密码框完成，失败时无需安装服务
                logWarn(`[DashboardView] macOS TUN 启动失败: ${res.error}`);
                toast.error("TUN 模式启动失败", `${res.error || "未知错误"}；已自动回退为系统代理模式`);
                rollback(prevActive, prevTun, prevMode);
              } else {
                const confirmInstall = confirm(
                  "启用 TUN 虚拟网卡需要管理员权限来安装静默提权组件。\n\n是否允许程序执行一键安装？(此后开启 TUN 将永久免弹窗免重启)"
                );
                if (confirmInstall) {
                  const installRes: ApiResponse = await invoke("service_install");
                  if (installRes.success) {
                    // 通过 proxyMode 参数避免安装后的二次 changeProxyMode 重启
                    const retryRes: ApiResponse = await invoke("tun_set_enabled", { enabled: true, proxyMode: "rule" });
                    if (retryRes.success) {
                      settingsStore.settings.tun_enabled = true;
                      settingsStore.settings.proxy_mode = "rule";
                      proxyStore.$patch({ proxyMode: "rule" });
                      return;
                    } else {
                      toast.error("TUN 模式启动失败", retryRes.error || "未知错误");
                    }
                  } else {
                    toast.error("提权组件安装失败", installRes.error || "未知错误");
                  }
                }
                rollback(prevActive, prevTun, prevMode);
              }
            }
          } else {
            // 非 TUN 模式：通过 proxyMode 参数避免二次 changeProxyMode 调用
            const proxyMode = settingsStore.settings.proxy_mode === "direct" ? "rule" : null;
            const res: ApiResponse = await invoke("tun_set_enabled", { enabled: false, proxyMode });
            if (res.success) {
              settingsStore.settings.tun_enabled = false;
              if (proxyMode) {
                settingsStore.settings.proxy_mode = proxyMode;
                proxyStore.$patch({ proxyMode });
              }
            } else {
              toast.error("代理启动失败", res.error || "未知错误");
              rollback(prevActive, prevTun, prevMode);
            }
          }
        } else {
          const res = await settingsStore.updateSettings({ tun_enabled: false, proxy_mode: "direct" });
          if (res.success) {
            settingsStore.settings.tun_enabled = false;
            proxyStore.$patch({ proxyMode: "direct" });
          } else {
            toast.error("关闭代理失败", res.error || "未知错误");
            rollback(prevActive, prevTun, prevMode);
          }
        }
      } catch (e) {
        console.error("开关代理错误: ", e);
        toast.error("开关代理异常", e instanceof Error ? e.message : String(e));
        rollback(prevActive, prevTun, prevMode); // 统一回滚
      } finally {
        operating.value = false;
        // 成功路径：操作完成即解除轮询保护窗口（10 秒兜底仍然有效）
        if (proxyActive.value === nextActive) {
          recentToggleUntil.value = 0;
        }
      }
    }, 50);
  }

  /** 切换分流模式（global / rule / direct） */
  function changeMode(mode: "global" | "rule" | "direct") {
    if (operating.value) return;
    operating.value = true;
    setTimeout(async () => {
      try {
        await proxyStore.changeProxyMode(mode);
      } finally {
        operating.value = false;
      }
    }, 50);
  }

  return {
    proxyActive,
    coreStarting,
    operating,
    recentToggleUntil,
    toggleProxy,
    changeMode,
  };
}
