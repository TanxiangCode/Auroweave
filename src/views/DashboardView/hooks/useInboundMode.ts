/**
 * 流量接管双态切换 Hook
 * 作者: TanXiang
 *
 * 职责：管理系统代理/TUN 虚拟网卡之间的切换
 */
import { computed, type Ref } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSettingsStore } from "@/stores/settings.store";
import { invoke } from "@tauri-apps/api/core";
import { info as logInfo, error as logError, warn as logWarn } from "@tauri-apps/plugin-log";
import type { ApiResponse } from "@/types";

interface UseInboundModeOptions {
  proxyActive: Ref<boolean>;
  operating: Ref<boolean>;
}

/**
 * 流量接管双态切换 Hook
 *
 * @param options - 依赖注入：代理激活状态与操作标志
 */
export function useInboundMode(options: UseInboundModeOptions) {
  const { proxyActive, operating } = options;
  const proxyStore = useProxyStore();
  const settingsStore = useSettingsStore();

  /** 流量接管双态读写双向绑定 */
  const inboundMode = computed({
    get() {
      return settingsStore.settings.tun_enabled ? "tun" : "system";
    },
    set(val: "system" | "tun") {
      if (operating.value) return;
      operating.value = true;
      logInfo(`[DashboardView] 用户点击切换网络接管模式为: ${val}`);

      const originalVal = settingsStore.settings.tun_enabled;
      const isTun = val === "tun";

      // 乐观更新
      settingsStore.settings.tun_enabled = isTun;

      setTimeout(async () => {
        try {
          if (!proxyActive.value) {
            logInfo(`[DashboardView] 代理未激活，仅标记设置 tun_enabled = ${isTun}`);
            await invoke("tun_set_enabled", { enabled: isTun });
            return;
          }

          if (val === "system") {
            logInfo("[DashboardView] 切换为普通系统代理模式，正在卸载 TUN...");
            // 通过 proxyMode 参数避免卸载 TUN 后二次 changeProxyMode 调用
            const proxyMode = settingsStore.settings.proxy_mode === "direct" ? "rule" : null;
            const res: ApiResponse = await invoke("tun_set_enabled", { enabled: false, proxyMode });
            if (res.success) {
              if (proxyMode) {
                settingsStore.settings.proxy_mode = proxyMode;
                proxyStore.$patch({ proxyMode });
              }
            } else {
              settingsStore.settings.tun_enabled = originalVal; // 回滚
            }
          } else if (val === "tun") {
            logInfo("[DashboardView] 切换为 TUN 虚拟网卡模式...");
            // 通过 proxyMode 参数一次性设置 tun_enabled + proxy_mode，
            // 避免先 tun_set_enabled 再 changeProxyMode 导致的第二次进程重启（macOS 第二次密码框）
            const proxyMode = settingsStore.settings.proxy_mode === "direct" ? "rule" : null;
            const res: ApiResponse = await invoke("tun_set_enabled", { enabled: true, proxyMode });
            if (!res.success) {
              const isMac = navigator.userAgent.toLowerCase().includes("mac");
              if (isMac) {
                // macOS: TUN 提权通过 osascript 密码框完成，失败时无需安装服务
                // 后端已自动回退为系统代理模式，仅提示用户即可
                logWarn(`[DashboardView] macOS TUN 启动失败: ${res.error}`);
                alert(`TUN 模式启动失败。\n\n${res.error}\n\n已自动回退为系统代理模式。`);
                settingsStore.settings.tun_enabled = originalVal; // 回滚
                return;
              }
              // Windows: 提示安装静默提权服务
              logWarn(`[DashboardView] 启动 TUN 失败: ${res.error}，提示用户一键提权安装服务...`);
              const confirmInstall = confirm(
                "启用 TUN 虚拟网卡需要管理员权限来安装静默提权组件。\n\n是否允许程序执行一键安装？(此后开启 TUN 将永久免弹窗免重启)"
              );
              if (confirmInstall) {
                logInfo("[DashboardView] 用户同意提权安装服务，开始调用 service_install...");
                const installRes: ApiResponse = await invoke("service_install");
                if (installRes.success) {
                  logInfo("[DashboardView] 服务安装成功，重新尝试启动 TUN...");
                  // 通过 proxyMode 参数避免安装后的二次 changeProxyMode 重启
                  const retryRes: ApiResponse = await invoke("tun_set_enabled", { enabled: true, proxyMode: "rule" });
                  if (retryRes.success) {
                    logInfo("[DashboardView] 重试启动 TUN 成功");
                    settingsStore.settings.tun_enabled = true;
                    settingsStore.settings.proxy_mode = "rule";
                    proxyStore.$patch({ proxyMode: "rule" });
                    return;
                  } else {
                    logError(`[DashboardView] 服务安装后，重试启动 TUN 依然失败: ${retryRes.error}`);
                  }
                } else {
                  logError(`[DashboardView] 提权服务安装失败: ${installRes.error}`);
                }
              }
              logWarn("[DashboardView] 启动 TUN 失败，静默回滚乐观状态");
              settingsStore.settings.tun_enabled = originalVal; // 回滚
              return;
            }
            logInfo("[DashboardView] 启动 TUN 成功");
            // 不再调用 changeProxyMode — tun_set_enabled 已用正确的 proxy_mode 启动 sing-box
            if (proxyMode) {
              settingsStore.settings.proxy_mode = proxyMode;
              proxyStore.$patch({ proxyMode });
            }
          }
        } catch (e: unknown) {
          logError(`[DashboardView] 切换接管模式发生致命错误: ${e instanceof Error ? e.message : e}`);
          settingsStore.settings.tun_enabled = originalVal; // 回滚
        } finally {
          operating.value = false;
        }
      }, 50);
    },
  });

  return { inboundMode };
}
