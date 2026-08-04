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
            const res: ApiResponse = await invoke("tun_set_enabled", { enabled: false });
            if (res.success) {
              if (proxyStore.proxyMode === "direct") {
                await proxyStore.changeProxyMode("rule");
              }
            } else {
              settingsStore.settings.tun_enabled = originalVal; // 回滚
            }
          } else if (val === "tun") {
            logInfo("[DashboardView] 切换为 TUN 虚拟网卡模式...");
            const res: ApiResponse = await invoke("tun_set_enabled", { enabled: true });
            if (!res.success) {
              logWarn(`[DashboardView] 启动 TUN 失败: ${res.error}，提示用户一键提权安装服务...`);
              const confirmInstall = confirm(
                "启用 TUN 虚拟网卡需要管理员权限来安装静默提权组件。\n\n是否允许程序执行一键安装？(此后开启 TUN 将永久免弹窗免重启)"
              );
              if (confirmInstall) {
                logInfo("[DashboardView] 用户同意提权安装服务，开始调用 service_install...");
                const installRes: ApiResponse = await invoke("service_install");
                if (installRes.success) {
                  logInfo("[DashboardView] 服务安装成功，重新尝试启动 TUN...");
                  const retryRes: ApiResponse = await invoke("tun_set_enabled", { enabled: true });
                  if (retryRes.success) {
                    logInfo("[DashboardView] 重试启动 TUN 成功");
                    await proxyStore.changeProxyMode("direct");
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
            await proxyStore.changeProxyMode("direct");
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
