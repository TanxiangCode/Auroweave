/**
 * Pinia Store — 应用设置
 * 作者: TanXiang
 */
import { defineStore } from "pinia";
import { ref, watch } from "vue";
import type { AppSettings } from "@/types";
import { getSettings, saveSettings } from "@/api/ipc/settings";

const DEFAULT_SETTINGS: AppSettings = {
  theme: "dark",
  language: "zh-CN",
  proxy_mode: "rule",
  auto_start: false,
  tun_enabled: false,
  topology_enabled: false,
  performance_mode: false,
  command_palette_hotkey: "CommandOrControl+Space",
  speed_test_urls: [
    "https://speed.cloudflare.com/__down?bytes=10000000",
    "https://fast.com",
  ],
  auto_group_on_import: true,

  mixed_port: 7890,
  clash_api_port: 9090,
  speed_test_url: "https://speed.cloudflare.com/__down?bytes=25000000",
  speed_test_timeout_secs: 5,
  connection_timeout_secs: 15,
};

export const useSettingsStore = defineStore("settings", () => {
  // ---- 状态 ----
  const settings = ref<AppSettings>({ ...DEFAULT_SETTINGS });
  const loaded = ref(false);

  // ---- 动作 ----
  async function fetchSettings() {
    const res = await getSettings();
    if (res.success && res.data) {
      settings.value = { ...DEFAULT_SETTINGS, ...res.data };
    }
    loaded.value = true;
    applyTheme(settings.value.theme);
    applyPerformanceMode(settings.value.performance_mode);
  }

  async function updateSettings(patch: Partial<AppSettings>) {
    const res = await saveSettings(patch);
    if (res.success) {
      Object.assign(settings.value, patch);
      if (patch.theme !== undefined) applyTheme(patch.theme);
      if (patch.performance_mode !== undefined)
        applyPerformanceMode(patch.performance_mode);
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

  return {
    settings,
    loaded,
    fetchSettings,
    updateSettings,
  };
});
