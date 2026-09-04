/**
 * 全局确认弹窗 composable
 * 作者: TanXiang
 *
 * 单例状态驱动 App.vue 挂载的唯一 ConfirmDialog 实例：
 *   const confirmed = await useConfirm().ask({
 *     title: "删除订阅",
 *     message: "此操作不可恢复。",
 *     level: "danger",
 *   });
 * 替换全部原生 confirm()（Tauri WebView 下样式与应用完全断裂的问题）。
 */
import { ref, readonly } from "vue";
import type { ConfirmOptions } from "@/components/common/ConfirmDialog.vue";

interface InternalConfirmOptions extends ConfirmOptions {
  defaultConfirmText?: string;
}

const visible = ref(false);
const title = ref("");
const message = ref("");
const confirmText = ref("确认");
const level = ref<"danger" | "normal">("danger");

let pendingResolve: ((ok: boolean) => void) | null = null;

/** 发起一次确认询问（同一时刻仅允许一个挂起确认） */
function ask(options: InternalConfirmOptions): Promise<boolean> {
  // 已有弹窗时直接返回 false（防重入），不排队——确认场景重入属异常路径
  if (visible.value) return Promise.resolve(false);

  title.value = options.title;
  message.value = options.message;
  confirmText.value = options.confirmText || options.defaultConfirmText || "确认";
  level.value = options.level || "danger";
  visible.value = true;

  return new Promise<boolean>((resolve) => {
    pendingResolve = resolve;
  });
}

function settle(ok: boolean) {
  visible.value = false;
  if (pendingResolve) {
    pendingResolve(ok);
    pendingResolve = null;
  }
}

export function useConfirm() {
  return {
    ask,
    // 供 ConfirmDialog 实例绑定（App.vue）
    state: {
      visible: readonly(visible),
      title: readonly(title),
      message: readonly(message),
      confirmText: readonly(confirmText),
      level: readonly(level),
    },
    settle,
  };
}
