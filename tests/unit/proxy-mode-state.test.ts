import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const proxyApi = vi.hoisted(() => ({
  setProxyMode: vi.fn(),
  getProxyMode: vi.fn(),
}));

const toast = vi.hoisted(() => ({
  success: vi.fn(),
  error: vi.fn(),
  info: vi.fn(),
  warning: vi.fn(),
}));

vi.mock("@/api/ipc/proxy", () => ({
  getProxyGroups: vi.fn(),
  getGroupNodes: vi.fn(),
  selectGroupNode: vi.fn(),
  setProxyMode: proxyApi.setProxyMode,
  getProxyMode: proxyApi.getProxyMode,
  updateGroupConfig: vi.fn(),
}));

vi.mock("@/composables/useToast", () => ({ useToast: () => toast }));
vi.mock("@/stores/unlock.store", () => ({
  useUnlockStore: () => ({ unlockMap: {} }),
}));

const storage = new Map<string, string>();
Object.defineProperty(globalThis, "localStorage", {
  configurable: true,
  value: {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
    removeItem: (key: string) => storage.delete(key),
    clear: () => storage.clear(),
  },
});

const { useProxyStore } = await import("@/stores/proxy.store");
const { useSettingsStore } = await import("@/stores/settings.store");

beforeEach(() => {
  setActivePinia(createPinia());
  storage.clear();
  proxyApi.setProxyMode.mockReset();
  proxyApi.getProxyMode.mockReset();
  toast.success.mockReset();
  toast.error.mockReset();
});

describe("代理模式状态同步", () => {
  it("切换成功时同时更新 proxyStore 与 settingsStore", async () => {
    proxyApi.setProxyMode.mockResolvedValue({ success: true });
    const proxyStore = useProxyStore();
    const settingsStore = useSettingsStore();

    const res = await proxyStore.changeProxyMode("global");

    expect(res.success).toBe(true);
    expect(proxyStore.proxyMode).toBe("global");
    expect(settingsStore.settings.proxy_mode).toBe("global");
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("切换失败时按 Clash API 实际模式回滚两个 Store", async () => {
    proxyApi.setProxyMode.mockResolvedValue({ success: false, error: "PATCH failed" });
    proxyApi.getProxyMode.mockResolvedValue({ success: true, data: "Rule" });
    const proxyStore = useProxyStore();
    const settingsStore = useSettingsStore();
    proxyStore.proxyMode = "rule";
    settingsStore.settings.proxy_mode = "rule";

    // 模拟设置页 v-model 已先改成 global，但内核实际仍为 Rule。
    settingsStore.settings.proxy_mode = "global";
    const res = await proxyStore.changeProxyMode("global");

    expect(res.success).toBe(false);
    expect(proxyStore.proxyMode).toBe("rule");
    expect(settingsStore.settings.proxy_mode).toBe("rule");
    expect(toast.error).toHaveBeenCalledWith("切换代理模式失败", "PATCH failed");
  });
});
