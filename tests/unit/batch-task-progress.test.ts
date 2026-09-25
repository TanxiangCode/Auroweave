/**
 * 批量任务进度状态机单测
 *
 * 覆盖三条批量任务（吞吐量测速 / 解锁检测）的前端状态机：
 * 启动即铺出 0/total 进度（否则底部进度条连同取消按钮会在首批探测期间缺席）、
 * 取消后忽略在途事件、终止事件统一复位。
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";

/** 后端进度事件回调的捕获位（mock 内注入，测试内手动触发） */
const hub = vi.hoisted(() => ({
  unlockProgressCb: null as null | ((p: any) => void),
  speedProgressCb: null as null | ((p: any) => void),
  latencyProgressCb: null as null | ((p: any) => void),
}));

vi.mock("@/composables/useToast", () => ({
  useToast: () => ({
    error: vi.fn(),
    success: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  }),
}));

vi.mock("@/api/ipc/unlock", () => ({
  runUnlockCheckSingle: vi.fn(),
  runUnlockCheckBatch: vi.fn(async () => ({ success: true })),
  cancelUnlockCheckBatch: vi.fn(async () => ({ success: true })),
  getLatestUnlockResults: vi.fn(async () => ({ success: true, data: [] })),
  listenUnlockCheckProgress: vi.fn(async (cb: (p: any) => void) => {
    hub.unlockProgressCb = cb;
    return () => {};
  }),
}));

vi.mock("@/api/ipc/speedtest", () => ({
  runLatencyTest: vi.fn(async () => ({ success: true, data: {} })),
  runSingleThroughputTest: vi.fn(),
  runBatchSpeedTest: vi.fn(async () => ({ success: true })),
  cancelBatchSpeedTest: vi.fn(async () => ({ success: true })),
  cancelLatencyTest: vi.fn(async () => ({ success: true })),
  getSpeedTestResults: vi.fn(async () => ({ success: true, data: {} })),
  listenSpeedTestProgress: vi.fn(async (cb: (p: any) => void) => {
    hub.speedProgressCb = cb;
    return () => {};
  }),
  listenLatencyTestProgress: vi.fn(async (cb: (p: any) => void) => {
    hub.latencyProgressCb = cb;
    return () => {};
  }),
}));

const { useUnlockStore } = await import("@/stores/unlock.store");
const { useSpeedtestStore } = await import("@/stores/speedtest.store");
const { runLatencyTest } = await import("@/api/ipc/speedtest");

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("批量解锁检测进度", () => {
  it("启动瞬间即有 0/total 进度，事件到达后推进", async () => {
    const store = useUnlockStore();
    await store.init();

    const started = store.startBatchCheck("proxy", ["a", "b", "c"]);
    expect(store.isBatchChecking).toBe(true);
    expect(store.batchProgress).toMatchObject({ current_index: 0, total: 3 });
    await started;

    hub.unlockProgressCb!({ current_index: 1, total: 3, current_node: "a" });
    expect(store.batchProgress?.current_index).toBe(1);
  });

  it("取消后冻结在途事件并由终止事件复位", async () => {
    const store = useUnlockStore();
    await store.init();
    await store.startBatchCheck("proxy", ["a", "b"]);
    hub.unlockProgressCb!({ current_index: 1, total: 2, current_node: "a" });

    await store.cancelBatch();
    expect(store.batchCancelled).toBe(true);

    hub.unlockProgressCb!({ current_index: 2, total: 2, current_node: "b" });
    // 进度冻结在取消瞬间（卡片改显"取消中"），取消按钮保持锁定直到终止事件
    expect(store.batchProgress?.current_index).toBe(1);
    expect(store.isBatchChecking).toBe(true);

    hub.unlockProgressCb!({ current_index: 2, total: 2, current_node: "" });
    expect(store.isBatchChecking).toBe(false);
    expect(store.batchProgress).toBe(null);
    expect(store.batchCancelled).toBe(false);
  });
});

describe("批量延迟测试进度", () => {
  it("完成后清进度，迟到事件只回填结果而不会复活 Dock", async () => {
    const store = useSpeedtestStore();
    await store.init();

    let resolveLatency: ((value: { success: boolean; data: Record<string, number> }) => void) | null = null;
    vi.mocked(runLatencyTest).mockImplementationOnce(
      () => new Promise((resolve) => { resolveLatency = resolve; })
    );

    const testing = store.testLatency("proxy", ["a", "b"]);
    expect(store.latencyBatchProgress).toMatchObject({ current_index: 0, total: 2 });

    hub.latencyProgressCb!({ current_index: 1, total: 2, current_node: "a", delay: 0 });
    expect(store.latencyMap.a).toBe(0);
    expect(store.latencyBatchProgress?.current_index).toBe(1);

    hub.latencyProgressCb!({ current_index: 2, total: 2, current_node: "", delay: 0 });
    expect(store.latencyBatchProgress).toBe(null);

    // 模拟前端保护性超时/迟到事件：不能再创建底部进度卡。
    hub.latencyProgressCb!({ current_index: 2, total: 2, current_node: "b", delay: 123 });
    expect(store.latencyMap.b).toBe(123);
    expect(store.latencyBatchProgress).toBe(null);

    resolveLatency!({ success: true, data: { a: 0, b: 123 } });
    await testing;
    expect(store.isTestingLatency).toBe(false);
  });

  it("取消后冻结在途事件，终止事件统一复位", async () => {
    const store = useSpeedtestStore();
    await store.init();

    let resolveLatency: ((value: { success: boolean; data: Record<string, number> }) => void) | null = null;
    vi.mocked(runLatencyTest).mockImplementationOnce(
      () => new Promise((resolve) => { resolveLatency = resolve; })
    );

    const testing = store.testLatency("proxy", ["a", "b"]);
    hub.latencyProgressCb!({ current_index: 1, total: 2, current_node: "a", delay: 10 });
    await store.cancelLatencyBatch();
    expect(store.latencyBatchCancelled).toBe(true);

    hub.latencyProgressCb!({ current_index: 2, total: 2, current_node: "b", delay: 0 });
    expect(store.latencyBatchProgress?.current_index).toBe(1);

    hub.latencyProgressCb!({ current_index: 2, total: 2, current_node: "", delay: 0 });
    expect(store.latencyBatchCancelled).toBe(false);
    expect(store.latencyBatchProgress).toBe(null);

    resolveLatency!({ success: true, data: { a: 10, b: 0 } });
    await testing;
  });
});

describe("批量吞吐量测速进度", () => {
  it("启动瞬间即有 0/total 进度，终止事件复位", async () => {
    const store = useSpeedtestStore();
    await store.init();

    const started = store.startBatchTest("proxy", ["a", "b"]);
    expect(store.isBatchTesting).toBe(true);
    expect(store.batchProgress).toMatchObject({ current_index: 0, total: 2 });
    await started;

    hub.speedProgressCb!({ current_index: 2, total: 2, current_node: "" });
    expect(store.isBatchTesting).toBe(false);
    expect(store.batchProgress).toBe(null);
  });
});
