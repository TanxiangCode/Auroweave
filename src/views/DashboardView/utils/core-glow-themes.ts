/**
 * 能量核主题色彩配置表
 * 作者: TanXiang
 *
 * 不同代理分流模式下的能量核光圈和文字阴影色彩配置
 */

export interface CoreGlowTheme {
  ring: {
    background: string;
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
      background: "conic-gradient(from 0deg, #f093fb, #f5576c, #ff9a9e, #f093fb)",
      boxShadow: "0 0 24px rgba(245, 87, 108, 0.45), inset 0 0 12px rgba(245, 87, 108, 0.3)",
    },
    text: {
      color: "#f093fb",
      textShadow: "0 0 8px rgba(240, 147, 251, 0.6)",
    },
    breathing: "rgba(240, 147, 251, 0.4)",
  },
  direct: {
    ring: {
      background: "conic-gradient(from 0deg, #43e97b, #38f9d7, #4facfe, #43e97b)",
      boxShadow: "0 0 24px rgba(67, 233, 123, 0.45), inset 0 0 12px rgba(67, 233, 123, 0.3)",
    },
    text: {
      color: "#43e97b",
      textShadow: "0 0 8px rgba(67, 233, 123, 0.6)",
    },
    breathing: "rgba(67, 233, 123, 0.4)",
  },
  rule: {
    ring: {
      background: "conic-gradient(from 0deg, #00f2fe, #4facfe, #f093fb, #00f2fe)",
      boxShadow: "0 0 24px rgba(0, 242, 254, 0.45), inset 0 0 12px rgba(0, 242, 254, 0.3)",
    },
    text: {
      color: "#00f2fe",
      textShadow: "0 0 8px rgba(0, 242, 254, 0.6)",
    },
    breathing: "rgba(0, 242, 254, 0.4)",
  },
};
