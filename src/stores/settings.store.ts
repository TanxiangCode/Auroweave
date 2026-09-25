/**
 * Pinia Store — 应用设置
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, watch } from "vue";
import type { AppSettings } from "@/types";
import { getSettings, saveSettings } from "@/api/ipc/settings";
import { useToast } from "@/composables/useToast";

import { DEFAULT_SPEED_TEST_URL, DEFAULT_SPEED_TEST_URLS } from "@/constants";

const DEFAULT_SETTINGS: AppSettings = {
  theme: "dark",
  language: "zh-CN",
  proxy_mode: "rule",
  auto_start: false,
  minimize_to_tray: true,

  start_minimized: false,
  hide_dock_on_close: false,
  show_tray_speed: true,
  allow_lan: false,

  tun_enabled: false,
  tun_interface_name: "Auroweave",
  topology_enabled: false,
  performance_mode: false,
  command_palette_hotkey: "CommandOrControl+Space",
  speed_test_urls: DEFAULT_SPEED_TEST_URLS,
  auto_group_on_import: true,
  enable_app_traffic_tracking: true,


  mixed_port: 8890,
  clash_api_port: 9090,
  speed_test_url: DEFAULT_SPEED_TEST_URL,
  speed_test_timeout_secs: 5,
  connection_timeout_secs: 15,
  latency_test_concurrency: 20,
  latency_test_timeout_ms: 5000,
  latency_test_url: "http://www.gstatic.com/generate_204",
  latency_unified_delay: false,
  latency_persistent_reuse: true,
  pinned_nodes: [],

  // DNS / TUN 进阶（sing-box 1.14.0）
  dns_mode: "fakeip",
  dns_remote_doh: "8.8.8.8",
  dns_timeout_secs: 5,
  dns_optimistic_cache: true,
  tun_dns_mode: "hijack",
  udp_nat_max: 0,
  core: {

    runMode: "local",
    service: {
      installedVersion: null,
      lastKnownStatus: "not_installed",
      lastFallbackReason: null,
    },
  },
};

export const useSettingsStore = defineStore("settings", () => {
  // ---- 状态 ----
  const settings = ref<AppSettings>({ ...DEFAULT_SETTINGS });
  const loaded = ref(false);

  // ---- 动作 ----
  async function fetchSettings() {
    try {
      const res = await getSettings();
      if (res.success && res.data) {
        settings.value = { ...DEFAULT_SETTINGS, ...res.data };
      } else {
        useToast().error("读取设置失败", res.error ?? "已回退到默认设置。");
      }
    } catch (e) {
      // IPC 异常（超时等）：loaded 仍需置位，避免界面永久卡在加载态
      console.error("读取设置异常:", e);
      useToast().error("读取设置失败", "与后端通信异常，已回退到默认设置。");
    } finally {
      loaded.value = true;
      applyTheme(settings.value.theme);
      applyPerformanceMode(settings.value.performance_mode);
    }
  }

  /**
   * 更新设置（部分更新语义）
   *
   * 后端 settings_save 为逐字段合并补丁（非整体覆盖），因此：
   * 1. 只提交本次修改的字段（patch 浅拷贝），绝不携带全量 settings —— 后端是
   *    read-modify-write 合并，全量提交会用本地可能过期的值覆盖其他写入口
   *    （托盘/调度器）刚写入的字段，造成丢失更新；
   * 2. 保存成功后再 Object.assign 合并到本地，保证 UI 即时生效。
   */
  async function updateSettings(patch: Partial<AppSettings>) {
    const res = await saveSettings({ ...patch });
    if (res.success) {
      Object.assign(settings.value, patch);
      if (patch.theme !== undefined) applyTheme(patch.theme);
      if (patch.performance_mode !== undefined)
        applyPerformanceMode(patch.performance_mode);
    } else {
      useToast().error("保存设置失败", res.error ?? "请检查后端服务是否正常运行。");
    }
    return res;
  }

  // ---- 主题注入（操作 <html> 上的 data-theme 属性） ----
  function applyTheme(theme: AppSettings["theme"]) {
    const root = document.documentElement;
    if (theme === "system") {
      const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      root.setAttribute("data-theme", isDark ? "dark" : "light");
    } else {
      root.setAttribute("data-theme", theme);
    }
  }

  // ---- 性能模式注入（操作 <html> 上的 data-perf-mode 属性） ----
  function applyPerformanceMode(enabled: boolean) {
    const root = document.documentElement;
    if (enabled) {
      root.setAttribute("data-perf-mode", "reduced");
    } else {
      root.removeAttribute("data-perf-mode");
    }
  }

  // 监听系统主题变化（仅在 theme = system 时生效）
  watch(
    () => settings.value.theme,
    (theme) => {
      if (theme === "system") {
        const mq = window.matchMedia("(prefers-color-scheme: dark)");
        const handler = () => applyTheme("system");
        mq.addEventListener("change", handler);
        return () => mq.removeEventListener("change", handler);
      }
    },
    { immediate: true }
  );

  // 监听 clash_api_port 变化，动态重连 WebSocket 客户端
  watch(
    () => settings.value.clash_api_port,
    () => {
      import("@/api/clash-ws").then(({ reconnectAll }) => {
        reconnectAll();
      });
    }
  );

  return {
    settings,
    loaded,
    fetchSettings,
    updateSettings,
  };
});
