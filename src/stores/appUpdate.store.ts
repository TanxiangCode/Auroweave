/**
 * Pinia Store — 应用本体（Auroweave）软件自更新
 * 作者: TanXiang
 *
 * 为什么必须放在 Store 而不是 AdvancedPanel 的局部 ref：
 * 与 `coreUpdate.store` 同理。自更新包含"下载 → 安装 → 重启"三段，
 * 用户在下载途中切到别的设置 tab 或路由页面时，局部状态会被组件卸载清空，
 * 按钮回到"检查更新"——用户会再点一次，触发重复下载（安装包动辄数十 MB）。
 * 状态上移到应用级单例后，切页面进度不丢、组件重挂载即刻复原。
 *
 * 阶段机（stage）：
 *   idle → checking → downloading → installing → ready（待重启） → 重启
 * 任一阶段失败进入 failed，携带 error 文案；成功后 ready 态常驻，
 * 直到用户重启（重启是不可撤销动作，不能自动做）。
 */
import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  checkAppUpdate,
  getAppVersion,
  installAppUpdate,
  relaunchApp,
} from "@/api/appUpdate";
import type { AppUpdateInfo } from "@/types";
import { useToast } from "@/composables/useToast";

/** 自更新阶段 */
export type AppUpdateStage =
  | "idle"
  | "checking"
  | "downloading"
  | "installing"
  | "ready"
  | "failed";

/** 下载进度（字节） */
interface DownloadProgress {
  downloaded: number;
  /** GitHub chunked 传输时可能为 0，UI 需据此隐藏百分比 */
  total: number;
}

/**
 * 插件 Update 对象的最小结构（只取本 store 需要的字段）
 *
 * 之所以自行声明而非 `import type { Update }`：单测在 `@/api/appUpdate`
 * 边界打桩，不应该为了一个类型而依赖真实的 Tauri 插件运行时。
 * 结构与插件保持一致即可。
 */
interface UpdateHandle {
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
  downloadAndInstall: (cb: (e: unknown) => void) => Promise<void>;
}

export const useAppUpdateStore = defineStore("appUpdate", () => {
  const currentVersion = ref<string>("...");
  const updateInfo = ref<AppUpdateInfo | null>(null);
  /**
   * 插件 Update 句柄（由 checkAppUpdate 返回）。
   *
   * 必须保存而非在 startUpdate 时重新 check：用户是在确认弹窗上看到
   * `updateInfo.version` 才点确认的，重新 check 会装上另一个版本
   * （"确认 0.8.0 却装上 0.8.1"），这是难以排查的信任问题。
   */
  const updateHandle = ref<UpdateHandle | null>(null);
  const stage = ref<AppUpdateStage>("idle");
  const progress = ref<DownloadProgress>({ downloaded: 0, total: 0 });
  const errorMessage = ref<string>("");

  /** 是否已检查过（区分"还没查"与"查了但失败"） */
  const checked = ref(false);

  /** 任务在途：checking 之外的下载/安装阶段都算 */
  const isBusy = computed(
    () => stage.value === "downloading" || stage.value === "installing"
  );

  /** 有新版本可装 */
  const hasUpdate = computed(() => updateInfo.value !== null);

  /** 安装完成、等待用户重启 */
  const awaitingRestart = computed(() => stage.value === "ready");

  /**
   * 下载百分比。
   *
   * `total === 0` 时返回 null 而不是 0：此时服务端没给 Content-Length，
   * 渲染 "0%" 会让用户以为卡死（内核升级踩过同样的坑）。
   */
  const percent = computed<number | null>(() => {
    const { downloaded, total } = progress.value;
    if (total <= 0) return null;
    return Math.min(100, Math.round((downloaded / total) * 100));
  });

  /** 按钮内的一行短文案 */
  const stageLabel = computed(() => {
    switch (stage.value) {
      case "checking":
        return "正在检查...";
      case "downloading":
        return percent.value !== null ? `下载中 ${percent.value}%` : "正在下载更新包";
      case "installing":
        return "正在安装...";
      case "ready":
        return "重启生效";
      case "failed":
        return "更新失败";
      default:
        return "";
    }
  });

  /** 进度条下方的细节文案 */
  const progressDetail = computed(() => {
    if (stage.value === "downloading" && progress.value.total > 0) {
      const mb = (n: number) => (n / 1024 / 1024).toFixed(1);
      return `${mb(progress.value.downloaded)} / ${mb(progress.value.total)} MB`;
    }
    return "";
  });

  function setStage(next: AppUpdateStage, err = "") {
    stage.value = next;
    errorMessage.value = err;
    if (next === "idle") {
      progress.value = { downloaded: 0, total: 0 };
    }
  }

  /** 读取当前版本号（面板挂载时调用） */
  async function fetchCurrentVersion() {
    const res = await getAppVersion();
    currentVersion.value = res.success && res.data ? res.data : "未知";
  }

  /**
   * 检查更新。
   *
   * 不自动弹 toast 打扰：自更新的触发频率低（面板内主动点），
   * 静默更新 UI 即可；只有失败才提示，避免"刚打开面板就被弹窗砸"。
   */
  async function checkUpdate(silent = false) {
    if (stage.value === "checking" || isBusy.value) return;
    setStage("checking");
    const res = await checkAppUpdate();
    if (!res.success) {
      setStage("failed", res.error || "无法连接更新服务器");
      if (!silent) useToast().error("检查更新失败", errorMessage.value);
      return;
    }
    checked.value = true;
    if (res.data) {
      updateInfo.value = res.data.info;
      updateHandle.value = res.data.update as unknown as UpdateHandle;
      setStage("idle");
      if (!silent) {
        useToast().info(
          `发现新版本 ${res.data.info.version}`,
          "点击「下载并安装」即可更新"
        );
      }
    } else {
      updateInfo.value = null;
      updateHandle.value = null;
      setStage("idle");
      if (!silent) {
        useToast().success("当前已是最新版本", `v${currentVersion.value}`);
      }
    }
  }

  /**
   * 下载并安装更新。
   *
   * 阶段推进：downloading（收到进度）→ installing（下载完成、
   * 插件正在落盘；此时不再有进度事件）→ ready（等待用户重启）。
   *
   * **不自动重启**：重启会中断进行中的代理连接与 TUN 网卡，
   * 必须由用户明确确认。
   */
  async function startUpdate() {
    if (isBusy.value || awaitingRestart.value) return;
    // 无句柄说明用户没检查过（或已 reset），此时不能凭空下载
    if (!updateHandle.value) {
      setStage("failed", "请先检查更新");
      return;
    }
    setStage("downloading");
    const res = await installAppUpdate(
      updateHandle.value as never,
      ({ downloaded, total }) => {
        // 安装阶段不再有进度事件，此处只在下载阶段更新
        if (stage.value === "downloading") {
          progress.value = { downloaded, total };
          // 进度已满 → 转入 installing，避免"下载 100% 后长时间无变化"，
          // 用户会以为卡住而重复点击（安装包动辄数十 MB）
          if (total > 0 && downloaded >= total) {
            stage.value = "installing";
          }
        }
      }
    );
    if (!res.success) {
      setStage("failed", res.error || "下载或安装失败");
      useToast().error("更新失败", errorMessage.value);
      return;
    }
    setStage("ready");
    useToast().success("更新已安装", "重启 Auroweave 后生效");
  }

  /** 重启应用使更新生效（不可撤销，调用前需用户确认） */
  async function restartNow() {
    const res = await relaunchApp();
    if (!res.success) {
      useToast().error("重启失败", res.error || "请手动重启应用");
    }
  }

  /** 复位到初始态（"重新检查"时调用） */
  function reset() {
    updateInfo.value = null;
    // 句柄必须一并清空：否则会拿着上一次的 Update 去下载，
    // 与界面上"待检查"的显示状态自相矛盾
    updateHandle.value = null;
    checked.value = false;
    setStage("idle");
  }

  return {
    currentVersion,
    updateInfo,
    stage,
    progress,
    errorMessage,
    checked,
    isBusy,
    hasUpdate,
    awaitingRestart,
    percent,
    stageLabel,
    progressDetail,
    fetchCurrentVersion,
    checkUpdate,
    startUpdate,
    restartNow,
    reset,
  };
});
