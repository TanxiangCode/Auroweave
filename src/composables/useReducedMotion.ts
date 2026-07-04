/**
 * Composable — useReducedMotion
 * 作者: TanXiang
 *
 * 统一判断是否应降级动效，同时兼容：
 * 1. 系统级 prefers-reduced-motion 媒体查询
 * 2. 软件内"性能模式"总开关（来自 Settings Store）
 *
 * 任一条件为真，所有装饰性动画均应降级为静态/简单过渡。
 */
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useSettingsStore } from "@/stores/settings.store";
import { storeToRefs } from "pinia";

export function useReducedMotion() {
  const settingsStore = useSettingsStore();
  const { settings } = storeToRefs(settingsStore);

  // 系统级偏好
  const systemReducedMotion = ref(false);

  let mediaQuery: MediaQueryList | null = null;
  const handleChange = (e: MediaQueryListEvent) => {
    systemReducedMotion.value = e.matches;
  };

  onMounted(() => {
    mediaQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    systemReducedMotion.value = mediaQuery.matches;
    mediaQuery.addEventListener("change", handleChange);
  });

  onUnmounted(() => {
    mediaQuery?.removeEventListener("change", handleChange);
  });

  /**
   * 是否应降级动效
   * - true  → 禁用流体/粒子/FLIP 等装饰性动画，使用静态或简单淡入替代
   * - false → 播放完整动效
   */
  const shouldReduceMotion = computed(
    () => systemReducedMotion.value || settings.value.performance_mode
  );

  return { shouldReduceMotion, systemReducedMotion };
}
