/**
 * 日志查看器 Hook
 * 作者: TanXiang
 *
 * 职责：日志获取、清空、导出诊断包
 */
import { ref, watch, onMounted, nextTick } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useToast } from "@/composables/useToast";
import { useConfirm } from "@/composables/useConfirm";
import { info as logInfo, error as logError } from "@tauri-apps/plugin-log";

type LogTab = "app" | "service";

/**
 * 日志查看器 Hook
 */
export function useLogViewer() {
  const toast = useToast();

  const activeTab = ref<LogTab>("app");
  const logContent = ref<string>("");
  const logViewer = ref<HTMLElement | null>(null);

  const tabs = [
    { key: "app" as LogTab, label: "应用与前端日志 (auroweave.log)" },
    { key: "service" as LogTab, label: "系统服务日志 (service.log)" },
  ];

  /** 获取日志内容 */
  async function fetchLogs() {
    const methodMap = {
      app: "log_read_app",
      service: "log_read_service",
    };
    const currentMethod = methodMap[activeTab.value];
    try {
      const res: any = await invoke(currentMethod, { lines: 250 });
      if (res.success) {
        logContent.value = res.data || "";
        nextTick(() => {
          if (logViewer.value) {
            logViewer.value.scrollTop = logViewer.value.scrollHeight;
          }
        });
      } else {
        logContent.value = `加载日志失败: ${res.error}`;
      }
    } catch (e: any) {
      logContent.value = `加载日志异常: ${e.message || e}`;
    }
  }

  /** 一键清理所有日志 */
  async function clearAllLogs() {
    const confirmed = await useConfirm().ask({
      title: "清空所有日志",
      message: "您确定要清空主程序日志、系统服务日志以及内核运行日志吗？此操作不可逆。",
      confirmText: "清空",
      level: "danger",
    });
    if (!confirmed) return;

    toast.info("正在清空所有日志...");
    try {
      const res: any = await invoke("log_clear_all");
      if (res.success) {
        toast.success("清空日志成功");
        logContent.value = "";
        logInfo("[PrivacyPanel] 成功清空了所有本地日志文件");
      } else {
        toast.error("清空日志失败", res.error);
      }
    } catch (e: any) {
      toast.error("清空日志失败", e.message || String(e));
    }
  }

  /** 导出诊断包 */
  async function handleExport() {
    logInfo("[PrivacyPanel] 用户尝试导出诊断包...");
    toast.info("正在生成诊断日志包...");
    try {
      const res: any = await invoke("settings_export_diagnostic_log");
      if (res.success) {
        logInfo(`[PrivacyPanel] 导出诊断包成功，路径: ${res.data}`);
        toast.success("导出成功", `已保存至: ${res.data || "桌面"}`);
      } else {
        logError(`[PrivacyPanel] 导出诊断包失败: ${res.error}`);
        toast.error("导出失败", res.error);
      }
    } catch (e: any) {
      logError(`[PrivacyPanel] 导出诊断包接口调用错误: ${e}`);
      toast.error("导出失败", e.message || String(e));
    }
  }

  watch(activeTab, () => {
    fetchLogs();
  });

  onMounted(() => {
    fetchLogs();
  });

  return {
    activeTab,
    logContent,
    logViewer,
    tabs,
    fetchLogs,
    clearAllLogs,
    handleExport,
  };
}
