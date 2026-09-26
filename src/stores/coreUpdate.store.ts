/**
 * Pinia Store — Sing-box 内核在线升级
 * 作者: TanXiang
 *
 * 为什么必须放在 Store 而不是 AdvancedPanel 的局部 ref：
 * 内核升级动辄数十 MB 下载 + 停核/替换/重启，历时数分钟。早期实现把
 * `upgrading` / `updateInfo` 挂在面板组件上，而该面板在 SettingsView 里
 * 由 v-else-if 渲染 —— 用户切到别的设置 tab 或别的路由页面时组件被卸载，
 * 进度归零；回到面板按钮又显示"立即在线升级"，用户会再点一次，
 * 触发真实的内核二进制替换（停核 → 覆盖 → 重启），代价极大。
 *
 * 现在状态上移到应用级单例，配合后端真值源形成两道保险：
 * 1. Store 常驻：切页面/切 tab 进度不丢，组件重挂载即刻复原；
 * 2. 后端槽位（core_upgrade_status）：即便整页重载导致 Store 重建，
 *    也能从后端回查真实的升级进度，不会误判为"空闲"而重复触发。
 */
import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  checkSingboxUpdate,
  getCoreUpgradeStatus,
  getSingboxVersion,
  listenCoreUpgradeProgress,
  upgradeSingbox,
} from "@/api/ipc/settings";
import type { SingboxUpdateInfo, SingboxUpdateProgress } from "@/types";
import { useToast } from "@/composables/useToast";

/** 升级结束（成功/失败）后结果态在 UI 上的停留时长，到期自动回落为 idle */
const RESULT_LINGER_MS = 8000;

/** idle 进度基线：字段与后端 SingboxUpdateProgress 对齐，避免 UI 侧到处判空 */
function idleProgress(): SingboxUpdateProgress {
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

export const useCoreUpdateStore = defineStore("coreUpdate", () => {
  /** 当前已安装内核版本（升级成功后需刷新） */
  const currentVersion = ref<string>("加载中...");
  /**
   * 内核是否具备 SUID root 权限。
   *
   * macOS 的 TUN 模式依赖它，而 fs::copy 替换内核必然丢失 SUID 位。
   * 早期实现只在后端打一条 warn 日志，用户侧完全无感——表现为
   * "升级完 TUN 莫名其妙起不来"。现在把它带到 UI 上主动提示。
   */
  const kernelPrivileged = ref<boolean>(true);
  /** GitHub Release 检测结果（含更新日志与下载资产） */
  const updateInfo = ref<SingboxUpdateInfo | null>(null);
  const checking = ref(false);
  const progress = ref<SingboxUpdateProgress>(idleProgress());

  let unlistenProgress: (() => void) | null = null;
  // init() 幂等哨兵：并发调用共享同一 Promise，防止重复注册事件监听
  let initPromise: Promise<void> | null = null;
  let resultTimer: ReturnType<typeof setTimeout> | null = null;
  /** 终态是否已播报：终态事件先于 invoke 返回到达，避免 toast 重复弹出 */
  let terminalNotified = false;

  /** 升级任务是否在途：按钮锁定 + 进度条显示的唯一依据 */
  const isUpgrading = computed(
    () => !progress.value.finished && progress.value.stage !== "idle"
  );

  /** 任务是否已终结（成功或失败），用于渲染结果态与解锁按钮 */
  const isFinished = computed(() => progress.value.finished);

  /** 按钮内的一行短文案（空间有限，只给阶段 + 百分比） */
  const stageLabel = computed(() => {
    const p = progress.value;
    if (!isUpgrading.value) return "";
    // 下载段仅在拿到 Content-Length 时才显示百分比：GitHub 走 chunked 传输时
    // total 为 0，此时若照常渲染会变成"已下 20MB 但进度显示 0%"的误导
    if (p.stage === "downloading" && p.total > 0) {
      return `正在下载 ${p.percent}%`;
    }
    return p.message.replace(/\.{3,}$/, "") || "正在升级内核";
  });

  /** 进度条下方的细节文案（下载段补百分比，便于确认是否卡住） */
  const progressDetail = computed(() => {
    const p = progress.value;
    if (!isUpgrading.value) return "";
    if (p.stage === "downloading" && p.total > 0) {
      return `${p.message} · ${p.percent}%`;
    }
    return p.message;
  });

  /** 终态提示文案（成功/失败），未终结时为空串 */
  const resultMessage = computed(() => {
    const p = progress.value;
    if (!p.finished) return "";
    return p.success ? "升级完成" : p.error || p.message || "升级失败";
  });

  /** 清理结果态定时器（新一轮升级前调用，避免旧定时器把新进度清成 idle） */
  function clearResultTimer() {
    if (resultTimer) {
      clearTimeout(resultTimer);
      resultTimer = null;
    }
  }


  /** 处理进度事件：更新状态，并在终态播报一次 toast */
  function applyProgress(p: SingboxUpdateProgress) {
    progress.value = p;

    if (!p.finished) {
      terminalNotified = false;
      clearResultTimer();
      return;
    }

    // 终态：结果态短暂停留后自动回落为 idle，
    // 免得面板上长期挂着一句"升级完成"
    clearResultTimer();
    resultTimer = setTimeout(() => {
      progress.value = idleProgress();
      resultTimer = null;
    }, RESULT_LINGER_MS);

    if (terminalNotified) return;
    terminalNotified = true;

    const toast = useToast();
    if (p.success) {
      toast.success("Sing-box 内核升级成功！", "新版本已自动替换并重新拉起运行");
      if (updateInfo.value) updateInfo.value.has_update = false;
      void fetchCurrentVersion();
    } else {
      toast.error("内核升级失败", p.error || "下载或替换过程中发生异常");
    }
  }

  /** 读取当前内核版本号与提权状态 */
  async function fetchCurrentVersion() {
    try {
      const res = await getSingboxVersion();
      if (res.success && res.data) {
        currentVersion.value = res.data.version;
        kernelPrivileged.value = res.data.privileged;
      } else {
        currentVersion.value = "未知";
      }
    } catch {
      currentVersion.value = "无法获取";
    }
  }

  /**
   * 初始化：注册进度监听 + 回查后端进度（幂等）
   *
   * 由 App.vue 在启动时调用，保证即使用户还没进过设置页，
   * 升级事件也不会漏接；回查则负责整页重载后的状态复原。
   */
  async function init() {
    if (initPromise) return initPromise;
    initPromise = (async () => {
      try {
        const res = await getCoreUpgradeStatus();
        // 仅在确有在途任务时覆盖，避免把上次的结果态复活成"正在升级"
        if (res.success && res.data && !res.data.finished && res.data.stage !== "idle") {
          progress.value = res.data;
        }
      } catch {
        // 回查失败不应阻塞 UI：事件流仍会驱动实时进度
      }

      if (!unlistenProgress) {
        unlistenProgress = await listenCoreUpgradeProgress((payload) => {
          applyProgress(payload);
        });
      }
    })();
    return initPromise;
  }

  /** 检查 GitHub Release 是否有新版本 */
  async function checkUpdate() {
    if (checking.value) return;
    checking.value = true;
    const toast = useToast();
    toast.info("正在查询 GitHub Release 最新版本...");
    try {
      const res = await checkSingboxUpdate();
      if (res.success && res.data) {
        updateInfo.value = res.data;
        if (res.data.has_update) {
          toast.info(
            `发现新版本 ${res.data.latest_version}`,
            "点击「立即在线升级」可一键自动更新内核"
          );
        } else {
          toast.success("当前已是最新内核版本", `v${res.data.current_version}`);
        }
      } else {
        toast.error("检查更新失败", res.error || "无法连接到 GitHub API");
      }
    } catch (e) {
      toast.error("检查更新失败", e instanceof Error ? e.message : String(e));
    } finally {
      checking.value = false;
    }
  }

  /**
   * 启动内核升级
   *
   * 后端命令是 fire-and-forget：这里只 await "启动确认"（白名单校验 + 闸门
   * 抢占），进度一律由事件流推进。因此调用方不应把本函数当作"升级完成"信号。
   */
  async function startUpgrade(downloadUrl: string) {
    if (isUpgrading.value) {
      useToast().warning("内核升级进行中", "请等待当前升级任务完成后再重试");
      return;
    }
    clearResultTimer();
    terminalNotified = false;

    // 立即铺出首帧进度：等后端第一条事件到达前按钮就要有反馈，
    // 否则点击后有几百毫秒的"点了没反应"空窗
    progress.value = {
      ...idleProgress(),
      stage: "preparing",
      message: "正在准备下载内核安装包...",
    };

    useToast().info("正在下载内核安装包并执行热替换，请稍候...");
    try {
      const res = await upgradeSingbox(downloadUrl);
      if (!res.success) {
        applyProgress({
          ...idleProgress(),
          stage: "failed",
          message: `内核升级失败: ${res.error || "启动升级任务失败"}`,
          finished: true,
          success: false,
          error: res.error || "启动升级任务失败",
        });
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      applyProgress({
        ...idleProgress(),
        stage: "failed",
        message: `内核升级失败: ${msg}`,
        finished: true,
        success: false,
        error: msg,
      });
    }
  }

  /** 手动清除结果态（供"重新检查"等操作立即复位 UI） */
  function clearResult() {
    clearResultTimer();
    progress.value = idleProgress();
  }

  return {
    currentVersion,
    kernelPrivileged,
    updateInfo,
    checking,
    progress,
    isUpgrading,
    isFinished,
    stageLabel,
    progressDetail,
    resultMessage,
    init,
    fetchCurrentVersion,
    checkUpdate,
    startUpgrade,
    clearResult,
  };
});

