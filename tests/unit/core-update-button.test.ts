// @vitest-environment happy-dom
/**
 * 内核升级进度 —— 组件级集成测试（真实 DOM + 真实点击 + 真实卸载/重挂）
 *
 * 上一份 store 单测只验证了状态机，本测试补上"进度真的显示在按钮上"
 * 以及"切走 tab 再回来进度还在"这两条用户可见的硬指标：
 *  - 点击「检查更新」→「立即在线升级」后，按钮内出现百分比与进度条；
 *  - 把面板组件整个 unmount（等价于 SettingsView 的 v-else-if 切走），
 *    再 mount 一个全新实例，进度依旧渲染 —— 这正是状态从组件局部 ref
 *    上移到 Pinia Store 之后才成立的行为；
 *  - 切到别的设置 tab 时，侧栏「高级与内核」条目上出现升级中角标。
 *
 * 仅在 IPC 边界（@/api/ipc/settings）打桩，组件、Store、计算属性全部跑真代码。
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { mount, flushPromises } from "@vue/test-utils";

const hub = vi.hoisted(() => ({
  progressCb: null as null | ((p: any) => void),
  statusResponse: null as null | { success: boolean; data?: any },
}));

function idleProgress() {
  return {
    stage: "idle",
    percent: 0,
    message: "",
    downloaded: 0,
    total: 0,
    finished: false,
    success: null,
    error: null,
  };
}

vi.mock("@/composables/useToast", () => ({
  useToast: () => ({
    toasts: [],
    show: vi.fn(),
    remove: vi.fn(),
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  }),
}));

vi.mock("@/composables/useConfirm", () => ({
  useConfirm: () => ({ ask: vi.fn(async () => true) }),
}));

vi.mock("@/api/ipc/settings", () => ({
  // settings.store 用到
  getSettings: vi.fn(async () => ({ success: true, data: {} })),
  saveSettings: vi.fn(async () => ({ success: true })),
  restoreConfigBackup: vi.fn(async () => ({ success: true })),
  // 面板 / coreUpdate.store 用到
  checkSingboxUpdate: vi.fn(async () => ({
    success: true,
    data: {
      current_version: "1.12.0",
      latest_version: "v1.13.0",
      has_update: true,
      release_notes: "## Changelog\n- 修复若干问题",
      published_at: "2026-09-20T10:00:00Z",
      download_url: "https://github.com/x/sing-box-1.13.0-darwin-arm64.tar.gz",
      download_size: 34_000_000,
    },
  })),
  upgradeSingbox: vi.fn(async () => ({ success: true })),
  getSingboxVersion: vi.fn(async () => ({ success: true, data: { version: "1.12.0", privileged: true, path: "/tmp/sing-box" } })),
  getCoreUpgradeStatus: vi.fn(async () => hub.statusResponse ?? { success: true, data: idleProgress() }),
  listenCoreUpgradeProgress: vi.fn(async (cb: (p: any) => void) => {
    hub.progressCb = cb;
    return () => {};
  }),
}));

vi.mock("vue-router", () => ({
  useRoute: () => ({ query: {}, path: "/settings" }),
  useRouter: () => ({ back: vi.fn() }),
}));

const AdvancedPanel = (await import("@/views/panels/AdvancedPanel.vue")).default;
const SettingsView = (await import("@/views/SettingsView.vue")).default;
const { useCoreUpdateStore } = await import("@/stores/coreUpdate.store");
const { getSingboxVersion } = await import("@/api/ipc/settings");

/** 挂载一个功能等价于 SettingsView 主区的面板实例 */
function mountPanel(pinia: any) {
  return mount(AdvancedPanel, { global: { plugins: [pinia] } });
}

/** 驱动后端下发一条进度事件（等价于 Rust 侧 emit_progress） */
function emitProgress(payload: Record<string, unknown>) {
  hub.progressCb!({ ...idleProgress(), ...payload });
}

beforeEach(() => {
  hub.progressCb = null;
  hub.statusResponse = null;
});

describe("内核升级进度在按钮上的显示", () => {
  it("点击升级后按钮内显示百分比与进度条", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mountPanel(pinia);
    await flushPromises();

    // 先点「检查更新」拿到新版本与下载资产
    await wrapper.find(".btn-check-update").trigger("click");
    await flushPromises();
    expect(wrapper.find(".update-release-box").exists()).toBe(true);

    // 再点「立即在线升级」
    const upgradeBtn = wrapper.find(".btn-upgrade-now");
    expect(upgradeBtn.text()).toContain("立即在线升级");
    await upgradeBtn.trigger("click");
    await flushPromises();

    // 点击瞬间就有反馈：按钮进入升级态、出现进度条轨道
    expect(wrapper.find(".btn-upgrade-now.is-running").exists()).toBe(true);
    expect(wrapper.find(".btn-progress-track").exists()).toBe(true);

    // 后端下发真实下载进度
    emitProgress({
      stage: "downloading",
      percent: 42,
      message: "正在下载内核安装包 (14.3 MB/34.0 MB)",
      downloaded: 14_300_000,
      total: 34_000_000,
    });
    await flushPromises();

    const running = wrapper.find(".btn-upgrade-now");
    expect(running.text()).toContain("正在下载 42%");
    // 进度条宽度由 percent 驱动
    expect(running.find(".btn-progress-fill").attributes("style")).toContain("width: 42%");
    // 细节行显示真实字节数
    expect(wrapper.find(".upgrade-progress-detail").text()).toContain("14.3 MB/34.0 MB");

    wrapper.unmount();
  });

  it("切走面板（组件卸载）再回来，进度不丢失", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const first = mountPanel(pinia);
    await flushPromises();

    await first.find(".btn-check-update").trigger("click");
    await flushPromises();
    await first.find(".btn-upgrade-now").trigger("click");
    await flushPromises();

    emitProgress({
      stage: "downloading",
      percent: 63,
      message: "正在下载内核安装包 (21.4 MB/34.0 MB)",
      downloaded: 21_400_000,
      total: 34_000_000,
    });
    await flushPromises();
    expect(first.find(".btn-upgrade-now").text()).toContain("正在下载 63%");

    // === 模拟用户切到别的设置 tab：面板被 v-else-if 卸载 ===
    first.unmount();

    // 后端继续推进（事件监听在 App 级 Store 中，未随组件销毁）
    emitProgress({ stage: "extracting", percent: 78, message: "正在解压内核安装包..." });

    // === 切回高级 tab：全新组件实例挂载 ===
    const second = mountPanel(pinia);
    await flushPromises();

    // 进度不仅还在，而且是"解压 78%"这一最新阶段
    const btn = second.find(".btn-upgrade-now");
    expect(btn.text()).toContain("正在解压内核安装包");
    expect(btn.find(".btn-progress-fill").attributes("style")).toContain("width: 78%");
    // 升级卡片没有被"无新版本"逻辑隐藏掉
    expect(second.find(".update-release-box").exists()).toBe(true);
    // 按钮锁定，不允许重复触发内核替换
    expect(btn.attributes("disabled")).toBeDefined();

    second.unmount();
  });

  it("切到别的设置 tab 时，侧栏「高级与内核」显示升级中角标", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const store = useCoreUpdateStore();
    await store.init();

    // 设置页停在「通用设置」，用户根本没打开高级面板
    const settings = mount(SettingsView, { global: { plugins: [pinia] } });
    await flushPromises();
    expect(settings.find(".update-release-box").exists()).toBe(false);
    expect(settings.find(".nav-updating-badge").exists()).toBe(false);

    // 升级任务在途（可由深链直达的面板发起）
    await store.startUpgrade("https://github.com/x/sing-box-1.13.0-darwin-arm64.tar.gz");
    emitProgress({ stage: "replacing", percent: 90, message: "正在替换内核文件..." });
    await flushPromises();

    // 角标出现在侧栏，进度不因"当前不在该 tab"而不可见
    const badge = settings.find(".nav-updating-badge");
    expect(badge.exists()).toBe(true);
    expect(badge.text()).toBe("90%");
    expect(badge.attributes("title")).toBe("正在替换内核文件...");

    settings.unmount();
  });

  it("升级成功后显示结果态并解锁按钮，不再显示进度", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mountPanel(pinia);
    await flushPromises();

    await wrapper.find(".btn-check-update").trigger("click");
    await flushPromises();
    await wrapper.find(".btn-upgrade-now").trigger("click");
    await flushPromises();

    emitProgress({
      stage: "done",
      percent: 100,
      message: "内核升级完成，已重新拉起运行",
      finished: true,
      success: true,
    });
    await flushPromises();

    const btn = wrapper.find(".btn-upgrade-now");
    // 进度条撤掉、按钮恢复可点，并展示成功态
    expect(btn.find(".btn-progress-track").exists()).toBe(false);
    expect(btn.text()).toContain("立即在线升级");
    expect(btn.attributes("disabled")).toBeUndefined();
    expect(wrapper.find(".upgrade-result.ok").exists()).toBe(true);
    expect(wrapper.find(".upgrade-result").text()).toContain("升级完成");

    wrapper.unmount();
  });

  it("macOS 上内核缺少 SUID 权限时给出 TUN 不可用告警", async () => {
    // 真实缺陷回归：升级替换内核必然丢 SUID，而早期实现只在后端打日志，
    // 用户侧完全无感——表现为"升级完 TUN 莫名其妙起不来"
    vi.mocked(getSingboxVersion).mockResolvedValueOnce({
      success: true,
      data: { version: "1.14.2", privileged: false, path: "/tmp/sing-box-1.14.2" },
    } as any);

    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mountPanel(pinia);
    await flushPromises();

    const warn = wrapper.find(".suid-warning");
    expect(warn.exists()).toBe(true, "应提示 TUN 模式不可用");
    expect(warn.text()).toContain("TUN 模式暂不可用");
    expect(warn.attributes("title")).toContain("TUN 网卡");

    wrapper.unmount();
  });

  it("SUID 正常时不显示告警", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mountPanel(pinia);
    await flushPromises();

    // mock 默认返回 privileged: true，不应出现告警噪音
    expect(wrapper.find(".suid-warning").exists()).toBe(false);

    wrapper.unmount();
  });

  it("升级失败落到失败终态，按钮解锁以便重试", async () => {
    const pinia = createPinia();
    setActivePinia(pinia);
    const wrapper = mountPanel(pinia);
    await flushPromises();

    await wrapper.find(".btn-check-update").trigger("click");
    await flushPromises();
    await wrapper.find(".btn-upgrade-now").trigger("click");
    await flushPromises();

    emitProgress({
      stage: "failed",
      percent: 40,
      message: "内核升级失败: 复制内核文件失败: 目标文件被占用",
      finished: true,
      success: false,
      error: "复制内核文件失败: 目标文件被占用",
    });
    await flushPromises();

    // 不能卡在"正在替换内核文件…"——那会让用户既看不到原因也无法重试
    expect(wrapper.find(".upgrade-result.fail").exists()).toBe(true);
    expect(wrapper.find(".upgrade-result").text()).toContain("目标文件被占用");
    expect(wrapper.find(".btn-upgrade-now").attributes("disabled")).toBeUndefined();

    wrapper.unmount();
  });
});
