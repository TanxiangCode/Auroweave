/**
 * 应用本体自更新状态机单测
 *
 * 覆盖自更新的核心风险点：
 * 1. `check()` 返回 null（已是最新）→ 不显示"发现新版本"，可重置后重查；
 * 2. 进度百分比在 `total === 0`（GitHub chunked 传输拿不到
 *    Content-Length）时必须返回 null 而不是 0 —— 否则 UI 会渲染
 *    "已下 20MB 但进度 0%"，用户以为卡死并重复点击重新下载；
 * 3. 下载 → 安装 → ready（待重启）的阶段推进，且**不会自动重启**
 *    （重启会打断代理连接/TUN，必须由用户确认）；
 * 4. 安装失败进入 failed 并携带原因，可重试。
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { watch } from "vue";

/** updater 插件行为的可编程替身 */
const hub = vi.hoisted(() => ({
  /** check() 的返回值：null 表示已是最新 */
  checkResult: null as null | {
    currentVersion: string;
    version: string;
    date?: string;
    body?: string;
  },
  /** downloadAndInstall 是否抛错 */
  installShouldFail: false,
  /** relaunch 是否被调用 */
  relaunchCalled: 0,
  /** check() 是否抛错（模拟断网） */
  checkShouldFail: false,
}));

vi.mock("@/composables/useToast", () => ({
  useToast: () => ({
    error: vi.fn(),
    success: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  }),
}));

vi.mock("@/api/appUpdate", () => ({
  getAppVersion: vi.fn(async () => ({ success: true, data: "0.7.0" })),
  checkAppUpdate: vi.fn(async () => {
    if (hub.checkShouldFail) {
      return { success: false, error: "无法连接更新服务器" };
    }
    if (!hub.checkResult) return { success: true, data: null };
    return {
      success: true,
      data: {
        // 插件 Update 句柄：真实的 downloadAndInstall 挂在它上面，
        // 单测用 hub 驱动而非真的下载
        update: hub.checkResult,
        info: {
          currentVersion: hub.checkResult.currentVersion,
          version: hub.checkResult.version,
          date: hub.checkResult.date ?? null,
          body: hub.checkResult.body ?? null,
        },
      },
    };
  }),
  installAppUpdate: vi.fn(
    async (
      _update: unknown,
      onProgress: (p: { downloaded: number; total: number }) => void
    ) => {
      if (hub.installShouldFail) {
        return { success: false, error: "下载失败：连接被重置" };
      }
      // 模拟有 Content-Length 的分块下载：1000 字节总量
      onProgress({ downloaded: 0, total: 1000 });
      onProgress({ downloaded: 400, total: 1000 });
      onProgress({ downloaded: 1000, total: 1000 });
      return { success: true };
    }
  ),
  relaunchApp: vi.fn(async () => {
    hub.relaunchCalled++;
    return { success: true };
  }),
}));

const { useAppUpdateStore } = await import("@/stores/appUpdate.store");

function makeCheck() {
  return {
    currentVersion: "0.7.0",
    version: "0.8.0",
    date: "2026-10-01T00:00:00Z",
    body: "## 变更\n- 新增自更新",
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  hub.checkResult = null;
  hub.installShouldFail = false;
  hub.relaunchCalled = 0;
  hub.checkShouldFail = false;
});

describe("appUpdate store — 检查更新", () => {
  it("初始为 idle，percent 为 null 而非 0", () => {
    const s = useAppUpdateStore();
    expect(s.stage).toBe("idle");
    // idle 时没有下载发生，百分比应为 null（不是 0）
    expect(s.percent).toBeNull();
    expect(s.hasUpdate).toBe(false);
    expect(s.checked).toBe(false);
  });

  it("check() 返回 null → 判定已是最新，不显示新版本", async () => {
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    expect(s.stage).toBe("idle");
    expect(s.hasUpdate).toBe(false);
    expect(s.checked).toBe(true);
  });

  it("发现新版本 → hasUpdate 为 true 且记录版本号与更新日志", async () => {
    hub.checkResult = makeCheck();
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    expect(s.hasUpdate).toBe(true);
    expect(s.updateInfo?.version).toBe("0.8.0");
    expect(s.updateInfo?.body).toContain("自更新");
    expect(s.stage).toBe("idle");
  });

  it("网络失败 → 进入 failed 并保留原因，可重试", async () => {
    hub.checkShouldFail = true;
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    expect(s.stage).toBe("failed");
    expect(s.errorMessage).toBe("无法连接更新服务器");
    s.reset();
    expect(s.stage).toBe("idle");
    expect(s.errorMessage).toBe("");
  });

  it("reset() 清空 updateInfo 与 checked，便于重新检查", async () => {
    hub.checkResult = makeCheck();
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    expect(s.checked).toBe(true);
    s.reset();
    expect(s.updateInfo).toBeNull();
    expect(s.checked).toBe(false);
  });

  it("未检查过就 startUpdate → 明确失败，不发下载请求", async () => {
    const s = useAppUpdateStore();
    // 没有 updateHandle，用户点不到按钮；这里直接调 store 兜底，
    // 防止任何路径绕过检查就发起数十 MB 下载
    await s.startUpdate();
    expect(s.stage).toBe("failed");
    expect(s.errorMessage).toBe("请先检查更新");
  });

  it("reset() 后 startUpdate 不复用上一次的句柄（所见即所装）", async () => {
    hub.checkResult = makeCheck();
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    expect(s.hasUpdate).toBe(true);
    s.reset();
    await s.startUpdate();
    // 关键：不应发出任何下载，否则会装上用户已经放弃的那个版本
    expect(s.stage).toBe("failed");
    expect(s.errorMessage).toBe("请先检查更新");
  });
});

describe("appUpdate store — 下载与安装", () => {
  it("下载进度推进：百分比随字节数增长", async () => {
    hub.checkResult = makeCheck();
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    await s.startUpdate();
    expect(s.progress.downloaded).toBe(1000);
    expect(s.percent).toBe(100);
  });

  it("安装成功后进入 ready（待重启），且不自动重启", async () => {
    hub.checkResult = makeCheck();
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    await s.startUpdate();
    expect(s.stage).toBe("ready");
    expect(s.awaitingRestart).toBe(true);
    // 关键安全约束：绝不能自动重启，会打断代理连接与 TUN
    expect(hub.relaunchCalled).toBe(0);
  });

  it("下载失败 → failed 携带原因，不进入 ready", async () => {
    hub.checkResult = makeCheck();
    hub.installShouldFail = true;
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    await s.startUpdate();
    expect(s.stage).toBe("failed");
    expect(s.awaitingRestart).toBe(false);
    expect(s.errorMessage).toContain("下载失败");
  });

  it("restartNow 才真正调用 relaunch", async () => {
    hub.checkResult = makeCheck();
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    await s.startUpdate();
    await s.restartNow();
    expect(hub.relaunchCalled).toBe(1);
  });
});

describe("appUpdate store — 阶段与文案", () => {
  it("total 为 0（chunked 传输）时 percent 返回 null 而非 0", () => {
    const s = useAppUpdateStore();
    // GitHub 走 chunked 传输时 contentLength 可能拿不到
    s.$patch({ stage: "downloading", progress: { downloaded: 20 * 1024 * 1024, total: 0 } });
    // 关键：不能是 0，否则 UI 显示"已下 20MB 但进度 0%"
    expect(s.percent).toBeNull();
    // 拿不到总量时文案退回"正在下载更新包"，不给假百分比
    expect(s.stageLabel).toBe("正在下载更新包");
  });

  it("stageLabel 覆盖各阶段文案", () => {
    const s = useAppUpdateStore();
    s.$patch({ stage: "checking" });
    expect(s.stageLabel).toBe("正在检查...");
    s.$patch({ stage: "installing" });
    expect(s.stageLabel).toBe("正在安装...");
    s.$patch({ stage: "ready" });
    expect(s.stageLabel).toBe("重启生效");
    s.$patch({ stage: "failed" });
    expect(s.stageLabel).toBe("更新失败");
  });

  it("isBusy 覆盖 downloading 与 installing，但不含 ready", () => {
    const s = useAppUpdateStore();
    s.$patch({ stage: "downloading" });
    expect(s.isBusy).toBe(true);
    s.$patch({ stage: "installing" });
    expect(s.isBusy).toBe(true);
    // ready 已装完，等用户重启，不应再显示"进行中"
    s.$patch({ stage: "ready" });
    expect(s.isBusy).toBe(false);
    expect(s.awaitingRestart).toBe(true);
  });

  it("下载阶段展示 MB 计数细节", () => {
    const s = useAppUpdateStore();
    s.$patch({ stage: "downloading", progress: { downloaded: 5 * 1024 * 1024, total: 10 * 1024 * 1024 } });
    expect(s.progressDetail).toBe("5.0 / 10.0 MB");
  });

  it("下载进度跑满后自动转入 installing（该阶段真实可达）", async () => {
    hub.checkResult = makeCheck();
    const s = useAppUpdateStore();
    await s.checkUpdate(true);
    // 捕获 installing 阶段：下载 100% 后、安装 resolve 前应短暂出现，
    // 否则 UI 会一直停在"下载 100%"，用户以为卡死而重复点击
    const seen: string[] = [];
    const stop = watch(
      () => s.stage,
      (v) => seen.push(v),
      { flush: "sync" }
    );
    await s.startUpdate();
    stop();
    expect(seen).toContain("installing");
  });
});

