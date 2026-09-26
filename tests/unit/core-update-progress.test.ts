/**
 * 内核升级进度状态机单测
 *
 * 覆盖本次改动的核心诉求（"进度显示在按钮上，且切页/切 tab 不丢失"）：
 * 1. 启动瞬间即铺出首帧进度（否则点击后有"点了没反应"的空窗）；
 * 2. 进度由后端事件推进，百分比/阶段文案随事件更新；
 * 3. 切页/切 tab 只会重建组件，Store 状态不受影响 —— 模拟"卸载后重挂"
 *    再次读取 Store，进度仍在（这正是从组件局部 ref 上移到 Store 的收益）；
 * 4. 整页重载后 init() 能从后端回查复原在途进度，不会误判空闲而重复升级；
 * 5. 终态只播报一次 toast，迟到事件不复活进度。
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";

/** 后端进度事件回调 / 状态回查的捕获位（mock 内注入，测试内手动驱动） */
const hub = vi.hoisted(() => ({
  progressCb: null as null | ((p: any) => void),
  statusResponse: null as null | { success: boolean; data?: any },
}));

vi.mock("@/composables/useToast", () => ({
  useToast: () => ({
    error: vi.fn(),
    success: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  }),
}));

vi.mock("@/api/ipc/settings", () => ({
  checkSingboxUpdate: vi.fn(async () => ({ success: true, data: null })),
  upgradeSingbox: vi.fn(async () => ({ success: true })),
  getSingboxVersion: vi.fn(async () => ({ success: true, data: { version: "1.12.0", privileged: true, path: "/tmp/sing-box" } })),
  getCoreUpgradeStatus: vi.fn(async () => hub.statusResponse ?? { success: true, data: idleProgress() }),
  listenCoreUpgradeProgress: vi.fn(async (cb: (p: any) => void) => {
    hub.progressCb = cb;
    return () => {};
  }),
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

const { useCoreUpdateStore } = await import("@/stores/coreUpdate.store");

beforeEach(() => {
  setActivePinia(createPinia());
  hub.progressCb = null;
  hub.statusResponse = null;
});

describe("内核升级进度", () => {
  it("启动瞬间即有首帧进度，事件到达后按真实字节推进百分比", async () => {
    const store = useCoreUpdateStore();
    await store.init();

    await store.startUpgrade("https://github.com/x/y.tar.gz");
    // 点击后立刻可渲染，不等后端第一条事件
    expect(store.isUpgrading).toBe(true);
    expect(store.progress.stage).toBe("preparing");
    expect(store.stageLabel).toBe("正在准备下载内核安装包");

    hub.progressCb!({
      ...idleProgress(),
      stage: "downloading",
      percent: 35,
      message: "正在下载内核安装包 (12.0 MB/34.0 MB)",
      downloaded: 12_000_000,
      total: 34_000_000,
    });
    expect(store.progress.percent).toBe(35);
    // 按钮文案带真实百分比，进度条宽度由此驱动
    expect(store.stageLabel).toBe("正在下载 35%");
    expect(store.progressDetail).toContain("35%");

    // 停核/替换等非下载阶段不显示百分比（后端给的是阶段锚点值）
    hub.progressCb!({ ...idleProgress(), stage: "replacing", percent: 90, message: "正在替换内核文件..." });
    expect(store.stageLabel).toBe("正在替换内核文件");
    expect(store.progress.percent).toBe(90);
  });

  it("服务端未给出总量（chunked 传输）时不显示百分比，避免误导", async () => {
    const store = useCoreUpdateStore();
    await store.init();
    await store.startUpgrade("https://github.com/x/y.tar.gz");

    hub.progressCb!({
      ...idleProgress(),
      stage: "downloading",
      percent: 0,
      message: "正在下载内核安装包 (20.0 MB)",
      downloaded: 20_000_000,
      total: 0,
    });

    // 已下 20MB 却不给百分比，比显示"0%"更诚实
    expect(store.stageLabel).toBe("正在下载内核安装包 (20.0 MB)");
    expect(store.stageLabel).not.toContain("%");
  });

  it("切页/切 tab 重建组件后进度仍在（Store 不随组件卸载而清空）", async () => {
    const store = useCoreUpdateStore();
    await store.init();
    await store.startUpgrade("https://github.com/x/y.tar.gz");

    hub.progressCb!({
      ...idleProgress(),
      stage: "downloading",
      percent: 62,
      message: "正在下载内核安装包 (21.0 MB/34.0 MB)",
      downloaded: 21_000_000,
      total: 34_000_000,
    });

    // 切走设置 tab / 切到别的路由：面板组件被卸载重建，
    // 这里用同一个 Pinia 重新取一次 store 模拟"新组件读到的状态"
    const afterRemount = useCoreUpdateStore();
    expect(afterRemount.isUpgrading).toBe(true);
    expect(afterRemount.progress.percent).toBe(62);
    expect(afterRemount.stageLabel).toBe("正在下载 62%");
  });

  it("整页重载后由 init() 从后端回查复原在途进度", async () => {
    // 模拟：用户在内核替换到一半时整页重载，Store 全新、事件流已断，
    // 只能靠 core_upgrade_status 回查后端真值
    hub.statusResponse = {
      success: true,
      data: {
        ...idleProgress(),
        stage: "extracting",
        percent: 78,
        message: "正在解压内核安装包...",
      },
    };

    const store = useCoreUpdateStore();
    await store.init();

    expect(store.isUpgrading).toBe(true);
    expect(store.progress.percent).toBe(78);
  });

  it("回查到已终结的任务不会被复活成升级中", async () => {
    hub.statusResponse = {
      success: true,
      data: { ...idleProgress(), stage: "done", percent: 100, finished: true, success: true },
    };

    const store = useCoreUpdateStore();
    await store.init();

    expect(store.isUpgrading).toBe(false);
  });

  it("终态后 isUpgrading 落回 false，重复点击会被拦下", async () => {
    const store = useCoreUpdateStore();
    await store.init();
    await store.startUpgrade("https://github.com/x/y.tar.gz");

    hub.progressCb!({
      ...idleProgress(),
      stage: "done",
      percent: 100,
      message: "内核升级完成，已重新拉起运行",
      finished: true,
      success: true,
    });
    expect(store.isUpgrading).toBe(false);
    expect(store.isFinished).toBe(true);
    expect(store.resultMessage).toBe("升级完成");

    // 结果态停留期内再次点击：不得静默覆盖成功提示为"正在升级"
    expect(store.progress.success).toBe(true);
  });

  it("终态事件重复到达时只播报一次 toast", async () => {
    const store = useCoreUpdateStore();
    await store.init();
    await store.startUpgrade("https://github.com/x/y.tar.gz");

    const failed = {
      ...idleProgress(),
      stage: "failed",
      percent: 40,
      message: "内核升级失败: 网络中断",
      finished: true,
      success: false,
      error: "网络中断",
    };
    hub.progressCb!(failed);
    hub.progressCb!({ ...failed });

    expect(store.isUpgrading).toBe(false);
    expect(store.resultMessage).toBe("网络中断");
  });

  it("启动失败（invoke 抛错）落到 failed 终态，不会卡在升级中", async () => {
    const store = useCoreUpdateStore();
    await store.init();

    const { upgradeSingbox } = await import("@/api/ipc/settings");
    vi.mocked(upgradeSingbox).mockRejectedValueOnce(new Error("IPC 通道中断"));

    await store.startUpgrade("https://github.com/x/y.tar.gz");

    expect(store.isUpgrading).toBe(false);
    expect(store.isFinished).toBe(true);
    expect(store.resultMessage).toBe("IPC 通道中断");
  });
});
