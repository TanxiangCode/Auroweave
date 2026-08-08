/**
 * 代理开关 Hook
 * 作者: TanXiang
 *
 * 职责：管理代理激活状态、乐观更新/回滚、分流模式切换
 */
import { ref } from "vue";
import { useProxyStore } from "@/stores/proxy.store";
import { useSettingsStore } from "@/stores/settings.store";
import { useSubscriptionStore } from "@/stores/subscription.store";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { info as logInfo, error as logError } from "@tauri-apps/plugin-log";
import type { ApiResponse } from "@/types";

/**
 * 代理开关 Hook
 *
 * 拥有 proxyActive 和 operating 状态，供其他 Hook 共享
 */
export function useProxyToggle() {
  const proxyStore = useProxyStore();
  const settingsStore = useSettingsStore();
  const subStore = useSubscriptionStore();
  const router = useRouter();

  /** 代理流量接管总开关 */
  const proxyActive = ref(true);
  /** 操作进行中标志（防重入） */
  const operating = ref(false);

  /** 一键接管网络 / 完全注销释放 Windows 代理 */
  function toggleProxy() {
    if (operating.value) return;
    operating.value = true;

    const nextActive = !proxyActive.value;

    // 启动代理前检查是否有可用订阅
    if (nextActive) {
      subStore.fetchAll().finally(() => {
        if (subStore.subscriptions.length === 0) {
          const confirmed = confirm(
            "尚未导入任何订阅，无法启动代理。\n\n是否前往设置页面导入订阅？"
          );
          if (confirmed) {
            router.push("/settings?panel=subscription");
          }
          operating.value = false;
          return;
        }
        proceedWithToggle(nextActive);
      });
      return;
    }

    proceedWithToggle(nextActive);
  }

  /** 实际执行代理开关逻辑 */
  function proceedWithToggle(nextActive: boolean) {
    // 乐观更新
    proxyActive.value = nextActive;

    setTimeout(async () => {
      try {
        if (nextActive) {
          if (settingsStore.settings.tun_enabled) {
            const res: ApiResponse = await invoke("tun_set_enabled", { enabled: true });
            if (res.success) {
              settingsStore.settings.tun_enabled = true;
              await proxyStore.changeProxyMode("direct");
            } else {
              const confirmInstall = confirm(
                "启用 TUN 虚拟网卡需要管理员权限来安装静默提权组件。\n\n是否允许程序执行一键安装？(此后开启 TUN 将永久免弹窗免重启)"
              );
              if (confirmInstall) {
                const installRes: ApiResponse = await invoke("service_install");
                if (installRes.success) {
                  const retryRes: ApiResponse = await invoke("tun_set_enabled", { enabled: true });
                  if (retryRes.success) {
                    settingsStore.settings.tun_enabled = true;
                    await proxyStore.changeProxyMode("direct");
                    return;
                  }
                }
              }
              proxyActive.value = false;
              settingsStore.settings.tun_enabled = false;
            }
          } else {
            await invoke("tun_set_enabled", { enabled: false });
            settingsStore.settings.tun_enabled = false;
            if (proxyStore.proxyMode === "direct") {
              await proxyStore.changeProxyMode("rule");
            }
          }
        } else {
          await settingsStore.updateSettings({ tun_enabled: false, proxy_mode: "direct" });
          settingsStore.settings.tun_enabled = false;
          proxyStore.$patch({ proxyMode: "direct" });
        }
      } catch (e) {
        console.error("开关代理错误: ", e);
        proxyActive.value = !nextActive; // 回滚
      } finally {
        operating.value = false;
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
    operating,
    toggleProxy,
    changeMode,
  };
}
