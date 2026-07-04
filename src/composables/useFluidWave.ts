/**
 * Composable — useFluidWave
 * 作者: TanXiang
 *
 * 驱动 Dashboard 中央能量核的流体动画：
 * - 用 requestAnimationFrame 驱动 conic-gradient 旋转
 * - 网速通过 EMA 平滑后映射到动画速度（见 connection.store.ts）
 * - 窗口不可见时自动暂停（Page Visibility API），避免后台空耗
 * - prefers-reduced-motion 或性能模式开启时，退化为静态渐变环
 */
import { ref, watch, onMounted, onUnmounted } from "vue";
import { useReducedMotion } from "./useReducedMotion";
import { ENERGY_MAX_SPEED_BPS } from "@/constants";

interface UseFluidWaveOptions {
  /** 当前 EMA 平滑后的下载速度（bytes/s），响应式 Ref */
  speedBps: Readonly<ReturnType<typeof ref<number>>>;
}

export function useFluidWave(options: UseFluidWaveOptions) {
  const { shouldReduceMotion } = useReducedMotion();

  /** 当前动画旋转角度（deg），绑定到 CSS 变量 */
  const rotationDeg = ref(0);
  /** 动画是否正在运行 */
  const isAnimating = ref(false);

  let rafId: number | null = null;
  let lastTimestamp: number | null = null;

  /**
   * 将网速映射到每秒旋转角度
   * - 静止：0 bps → 30 deg/s（缓慢呼吸旋转）
   * - 满速：ENERGY_MAX_SPEED_BPS → 360 deg/s（一秒一圈）
   */
  function speedToRotationRate(bps: number): number {
    const clampedRatio = Math.min(bps / ENERGY_MAX_SPEED_BPS, 1);
    return 30 + clampedRatio * 330; // 30~360 deg/s
  }

  function tick(timestamp: number) {
    if (shouldReduceMotion.value) {
      stop();
      return;
    }

    if (lastTimestamp === null) lastTimestamp = timestamp;
    const delta = (timestamp - lastTimestamp) / 1000; // 转秒
    lastTimestamp = timestamp;

    const rate = speedToRotationRate(options.speedBps.value ?? 0);
    rotationDeg.value = (rotationDeg.value + rate * delta) % 360;

    rafId = requestAnimationFrame(tick);
  }

  function start() {
    if (rafId !== null || shouldReduceMotion.value) return;
    isAnimating.value = true;
    lastTimestamp = null;
    rafId = requestAnimationFrame(tick);
  }

  function stop() {
    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
    }
    isAnimating.value = false;
    lastTimestamp = null;
  }

  // 页面可见性监听（最小化到托盘时暂停）
  function handleVisibilityChange() {
    if (document.hidden) {
      stop();
    } else {
      start();
    }
  }

  // 性能模式/系统偏好变化时响应
  watch(shouldReduceMotion, (reduced) => {
    if (reduced) {
      stop();
    } else if (!document.hidden) {
      start();
    }
  });

  onMounted(() => {
    document.addEventListener("visibilitychange", handleVisibilityChange);
    if (!shouldReduceMotion.value && !document.hidden) {
      start();
    }
  });

  onUnmounted(() => {
    stop();
    document.removeEventListener("visibilitychange", handleVisibilityChange);
  });

  return {
    rotationDeg,
    isAnimating,
    shouldReduceMotion,
  };
}
