/**
 * 能量核主题色彩配置表
 * 作者: TanXiang
 *
 * 不同代理分流模式下的能量核光圈和文字阴影色彩配置
 *
 * 动画效果参考小米充电动画：
 * - 圆环大部分暗淡（仅保留极低透明度的底色）
 * - 一段亮色"彗星"光带沿圆环旋转（头部最亮，尾部渐隐）
 * - 亮色光带通过 conic-gradient 的角度区间控制
 */

export interface CoreGlowTheme {
  ring: {
    /** 旋转彗星光带的 conic-gradient（头部最亮 → 尾部渐隐 → 大段透明） */
    background: string;
    /** 圆环暗色底色（始终可见的低透明度主色） */
    ringBase: string;
    boxShadow: string;
  };
  text: {
    color: string;
    textShadow: string;
  };
  breathing: string;
}

/** 各分流模式对应的能量核主题色彩 */
export const CORE_GLOW_THEMES: Record<string, CoreGlowTheme> = {
  global: {
    ring: {
      background:
        "conic-gradient(from 0deg, transparent 0deg, transparent 200deg, rgba(240, 147, 251, 0.04) 220deg, rgba(240, 147, 251, 0.12) 250deg, rgba(245, 87, 108, 0.28) 280deg, rgba(240, 147, 251, 0.6) 310deg, #f093fb 340deg, rgba(240, 147, 251, 0.4) 355deg, transparent 360deg)",
      ringBase: "rgba(240, 147, 251, 0.06)",
      boxShadow:
        "0 0 24px rgba(245, 87, 108, 0.45), inset 0 0 12px rgba(245, 87, 108, 0.3)",
    },
    text: {
      color: "#f093fb",
      textShadow: "0 0 8px rgba(240, 147, 251, 0.6)",
    },
    breathing: "rgba(240, 147, 251, 0.4)",
  },
  direct: {
    ring: {
      background:
        "conic-gradient(from 0deg, transparent 0deg, transparent 200deg, rgba(67, 233, 123, 0.04) 220deg, rgba(67, 233, 123, 0.12) 250deg, rgba(56, 249, 215, 0.28) 280deg, rgba(67, 233, 123, 0.6) 310deg, #43e97b 340deg, rgba(67, 233, 123, 0.4) 355deg, transparent 360deg)",
      ringBase: "rgba(67, 233, 123, 0.06)",
      boxShadow:
        "0 0 24px rgba(67, 233, 123, 0.45), inset 0 0 12px rgba(67, 233, 123, 0.3)",
    },
    text: {
      color: "#43e97b",
      textShadow: "0 0 8px rgba(67, 233, 123, 0.6)",
    },
    breathing: "rgba(67, 233, 123, 0.4)",
  },
  rule: {
    ring: {
      background:
        "conic-gradient(from 0deg, transparent 0deg, transparent 200deg, rgba(0, 242, 254, 0.04) 220deg, rgba(0, 242, 254, 0.12) 250deg, rgba(79, 172, 254, 0.28) 280deg, rgba(0, 242, 254, 0.6) 310deg, #00f2fe 340deg, rgba(0, 242, 254, 0.4) 355deg, transparent 360deg)",
      ringBase: "rgba(0, 242, 254, 0.06)",
      boxShadow:
        "0 0 24px rgba(0, 242, 254, 0.45), inset 0 0 12px rgba(0, 242, 254, 0.3)",
    },
    text: {
      color: "#00f2fe",
      textShadow: "0 0 8px rgba(0, 242, 254, 0.6)",
    },
    breathing: "rgba(0, 242, 254, 0.4)",
  },
};
