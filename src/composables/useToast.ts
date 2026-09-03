/**
 * Toast 全局消息通知 Composable
 * 作者: TanXiang
 */
import { ref } from "vue";

export interface ToastMessage {
  id: string;
  type: "success" | "info" | "warning" | "error";
  title: string;
  message?: string;
  duration?: number;
}

const toasts = ref<ToastMessage[]>([]);

/** 同屏 Toast 堆叠上限：超出时移除最老的一条，防止错误风暴刷屏 */
const TOAST_MAX_VISIBLE = 5;

export function useToast() {
  const show = (toast: Omit<ToastMessage, "id">) => {
    const id = Math.random().toString(36).substring(2, 9);
    const newToast: ToastMessage = {
      id,
      duration: 3500,
      ...toast,
    };
    toasts.value.push(newToast);

    // 超过上限时用 splice 移除最老的（保持数组引用不变，保留既有过渡动画语义）
    const overflow = toasts.value.length - TOAST_MAX_VISIBLE;
    if (overflow > 0) {
      toasts.value.splice(0, overflow);
    }

    if (newToast.duration && newToast.duration > 0) {
      setTimeout(() => {
        remove(id);
      }, newToast.duration);
    }
  };

  const remove = (id: string) => {
    toasts.value = toasts.value.filter((t) => t.id !== id);
  };

  const success = (title: string, message?: string) => show({ type: "success", title, message });
  const error = (title: string, message?: string) => show({ type: "error", title, message });
  const info = (title: string, message?: string) => show({ type: "info", title, message });
  const warning = (title: string, message?: string) => show({ type: "warning", title, message });

  return {
    toasts,
    show,
    remove,
    success,
    error,
    info,
    warning,
  };
}
