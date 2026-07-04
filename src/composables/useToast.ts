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

export function useToast() {
  const show = (toast: Omit<ToastMessage, "id">) => {
    const id = Math.random().toString(36).substring(2, 9);
    const newToast: ToastMessage = {
      id,
      duration: 3500,
      ...toast,
    };
    toasts.value.push(newToast);

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
