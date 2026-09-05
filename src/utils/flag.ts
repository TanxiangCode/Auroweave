/**
 * 国旗渲染工具
 * 作者: TanXiang
 *
 * ISO 3166-1 alpha-2 国家/地区代码 → 本地内联 SVG（离线渲染，无外链请求）。
 * 采用经典 CSS sprite 技巧：以 shared SVG symbol + viewBox 平移复用一张
 * 网格图？不——为保证每个国旗视觉独立且无大位图依赖，这里按代码查表
 * 渲染为统一尺寸的简单矩形条纹/底色近似旗（核心场景是"归属地小图标"
 * 的视觉辨识而非精确旗面，节点出口常见国家覆盖精确版）。
 *
 * 若代码不在精确表中，回退为「地球图标 + 代码文字」，保证任何归属地都有反馈。
 */

/** 精确旗面定义（横条纹/纵条纹/纯色，颜色为该国旗的主识别色） */
interface FlagSpec {
  /** 渲染方式：h=水平条纹 v=垂直条纹 s=纯色 d=特殊自定义 path */
  type: "h" | "v" | "s" | "d";
  /** 条纹/底色序列（type=s 时仅取首色） */
  colors: string[];
  /** 特殊旗面的自定义元素（d 类型时使用） */
  extra?: string;
}

/** 出口节点常见国家/地区的精确旗面（不够全量 249 面，覆盖代理出口高频区） */
const FLAGS: Record<string, FlagSpec> = {
  JP: { type: "s", colors: ["#f5f5f5"], extra: `<circle cx="24" cy="18" r="8" fill="#bc002d"/>` },
  US: { type: "h", colors: ["#b22234", "#fff", "#b22234", "#fff", "#b22234", "#fff", "#b22234", "#fff", "#b22234", "#fff", "#b22234", "#fff", "#b22234"] },
  HK: { type: "d", colors: ["#de2910"], extra: `<circle cx="20" cy="12" r="6" fill="#ffde00"/><circle cx="14" cy="8" r="1.2" fill="#ffde00"/><circle cx="14" cy="16" r="1.2" fill="#ffde00"/><circle cx="10" cy="12" r="1.2" fill="#ffde00"/>` },
  TW: { type: "d", colors: ["#fe0000"], extra: `<rect x="0" y="0" width="24" height="12" fill="#000095"/><circle cx="10" cy="6" r="3.5" fill="#fff"/><circle cx="10" cy="6" r="2" fill="#000095"/>` },
  SG: { type: "h", colors: ["#ed2939", "#fff"], extra: `<circle cx="10" cy="12" r="4.5" fill="#fff"/><circle cx="12" cy="12" r="4" fill="#ed2939"/><circle cx="14" cy="12" r="1.4" fill="#fff"/>` },
  KR: { type: "d", colors: ["#fff"], extra: `<circle cx="24" cy="18" r="5" fill="#cd2e3a"/><circle cx="21.5" cy="18" r="5" fill="#0047a0"/><circle cx="24" cy="18" r="2.5" fill="#cd2e3a"/>` },
  GB: { type: "d", colors: ["#012169"], extra: `<path d="M0,0 L48,36 L48,30 L8,0 Z" fill="#fff"/><path d="M48,0 L0,30 L0,36 L40,0 Z" fill="#fff"/><path d="M0,0 L48,36 M48,0 L0,36" stroke="#fff" stroke-width="7"/><path d="M0,0 L48,36 M48,0 L0,36" stroke="#c8102e" stroke-width="4"/><path d="M24,0 V36 M0,18 H48" stroke="#fff" stroke-width="12"/><path d="M24,0 V36 M0,18 H48" stroke="#c8102e" stroke-width="7"/>` },
  DE: { type: "h", colors: ["#000", "#dd0000", "#ffce00"] },
  FR: { type: "v", colors: ["#0055a4", "#fff", "#ef4135"] },
  NL: { type: "h", colors: ["#ae1c28", "#fff", "#21468b"] },
  CA: { type: "v", colors: ["#d80621", "#fff", "#d80621", "#fff", "#d80621"], extra: `<rect x="18" y="8" width="12" height="20" fill="#d80621"/>` },
  AU: { type: "s", colors: ["#012169"], extra: `<circle cx="14" cy="8" r="2.5" fill="#fff"/><circle cx="20" cy="14" r="2" fill="#fff"/><circle cx="34" cy="6" r="2" fill="#fff"/><circle cx="38" cy="20" r="2.5" fill="#fff"/><circle cx="28" cy="28" r="2" fill="#fff"/>` },
  RU: { type: "h", colors: ["#fff", "#0039a6", "#d52b1e"] },
  IN: { type: "h", colors: ["#ff9933", "#fff", "#138808"], extra: `<circle cx="24" cy="18" r="3.5" fill="none" stroke="#000080" stroke-width="1"/>` },
  TH: { type: "h", colors: ["#a51931", "#f4f5f7", "#2d2a4a", "#f4f5f7", "#a51931"] },
  VN: { type: "s", colors: ["#da251d"], extra: `<path d="M24 9 L26.5 16 L34 16 L28 20.5 L30.5 27.5 L24 23 L17.5 27.5 L20 20.5 L14 16 L21.5 16 Z" fill="#ff0"/>` },
  MY: { type: "h", colors: ["#010066", "#fff", "#cc0001", "#fff", "#010066"], extra: `<circle cx="24" cy="9" r="4" fill="#ffcc00"/><circle cx="24" cy="7" r="4" fill="#010066"/>` },
  PH: { type: "h", colors: ["#0038a8", "#fff", "#0038a8"], extra: `<path d="M0,0 L24,18 L0,36 Z" fill="#fff"/><path d="M0,0 L24,18 L0,36 Z" fill="#0038a8"/>` },
  TR: { type: "s", colors: ["#e30a17"], extra: `<circle cx="21" cy="18" r="6" fill="#e30a17"/><circle cx="23" cy="18" r="4.8" fill="#fff"/><path d="M27 18 L23.8 17 L24 20 Z" fill="#e30a17"/>` },
  BR: { type: "s", colors: ["#009c3b"], extra: `<path d="M0,0 L48,18 L0,36 Z" fill="#ffdf00"/><circle cx="15" cy="18" r="6" fill="#002776"/><path d="M15 12.5 a 5.5 5.5 0 0 1 0 11 a 4.5 4.5 0 0 0 0 -11" fill="#fff"/>` },
  AE: { type: "h", colors: ["#00732f", "#fff", "#000", "#fff", "#ff0000"] },
  SA: { type: "s", colors: ["#006c35"], extra: `<rect x="0" y="0" width="36" height="27" fill="#006c35"/>` },
  IT: { type: "v", colors: ["#009246", "#fff", "#ce2b37"] },
  ES: { type: "h", colors: ["#aa151b", "#f1bf00", "#aa151b"] },
  CH: { type: "s", colors: ["#d52b1e"], extra: `<rect x="0" y="0" width="48" height="10" fill="#d52b1e"/><rect x="0" y="26" width="48" height="10" fill="#d52b1e"/><path d="M24 13 L25.5 17 L29.5 17 L26.2 19.4 L27.5 23 L24 20.7 L20.5 23 L21.8 19.4 L18.5 17 L22.5 17 Z" fill="#fff"/>` },
  SE: { type: "s", colors: ["#006aa7"], extra: `<rect x="0" y="0" width="48" height="36" fill="#006aa7"/><rect x="12" y="0" width="7" height="36" fill="#fecc00"/><rect x="0" y="14" width="48" height="7" fill="#fecc00"/>` },
  NO: { type: "s", colors: ["#ba0c2f"], extra: `<rect x="0" y="0" width="48" height="36" fill="#ba0c2f"/><rect x="12" y="0" width="7" height="36" fill="#fff"/><rect x="0" y="14" width="48" height="7" fill="#fff"/><rect x="14" y="0" width="3" height="36" fill="#00205b"/><rect x="0" y="16" width="48" height="3" fill="#00205b"/>` },
  UA: { type: "h", colors: ["#005bbb", "#ffd500"] },
  IE: { type: "v", colors: ["#169b62", "#fff", "#ff883e"] },
  ID: { type: "h", colors: ["#ce1126", "#fff"] },
  MX: { type: "v", colors: ["#006847", "#fff", "#ce1126"], extra: `<circle cx="24" cy="18" r="3" fill="#8c6a3f"/>` },
  AR: { type: "h", colors: ["#74acdf", "#fff", "#74acdf"], extra: `<circle cx="24" cy="18" r="3.2" fill="#f6b40e"/>` },
  CN: { type: "s", colors: ["#de2910"], extra: `<path d="M14 5 L15.4 9.2 L19.8 9.2 L16.2 11.8 L17.6 16 L14 13.4 L10.4 16 L11.8 11.8 L8.2 9.2 L12.6 9.2 Z" fill="#ffde00"/><circle cx="22" cy="4" r="1" fill="#ffde00"/><circle cx="25" cy="7" r="1" fill="#ffde00"/><circle cx="25" cy="11" r="1" fill="#ffde00"/><circle cx="22" cy="13.5" r="1" fill="#ffde00"/>` },
};

/** 国家代码 → 小写 ISO 代码（供 SVG id 与防注入校验） */
function normalizeCode(code: string): string {
  const c = code.trim().toUpperCase();
  return /^[A-Z]{2}$/.test(c) ? c : "";
}

/**
 * 生成国旗内联 SVG 字符串（可直接 v-html 进 span）
 *
 * 未收录代码回退为「代码文字徽章」样式（地球占位由调用方 CSS 兜底）。
 */
export function renderFlagSvg(code: string): string {
  const c = normalizeCode(code);
  const spec = c ? FLAGS[c] : undefined;
  const W = 48;
  const H = 36;

  if (!spec) {
    // 未收录：灰底 + 代码文字（保证任何归属地都有视觉反馈）
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 36" class="flag-svg"><rect width="48" height="36" rx="3" fill="rgba(255,255,255,0.08)"/><text x="24" y="22" font-size="13" font-weight="700" text-anchor="middle" fill="rgba(255,255,255,0.65)" font-family="var(--font-mono, monospace)">${escapeHtml(c || "??")}</text></svg>`;
  }

  let body = "";
  if (spec.type === "s" || spec.type === "d") {
    body = `<rect width="${W}" height="${H}" fill="${spec.colors[0]}"/>${spec.extra ?? ""}`;
  } else if (spec.type === "h") {
    const stripH = H / spec.colors.length;
    body =
      spec.colors
        .map((col, i) => `<rect x="0" y="${(i * stripH).toFixed(2)}" width="${W}" height="${stripH.toFixed(2)}" fill="${col}"/>`)
        .join("") + (spec.extra ?? "");
  } else {
    const stripW = W / spec.colors.length;
    body =
      spec.colors
        .map((col, i) => `<rect x="${(i * stripW).toFixed(2)}" y="0" width="${stripW.toFixed(2)}" height="${H}" fill="${col}"/>`)
        .join("") + (spec.extra ?? "");
  }

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 36" class="flag-svg" preserveAspectRatio="xMidYMid slice">${body}</svg>`;
}

/** 代码是否已收录精确旗面（未收录时调用方可选择只展示文字） */
export function isKnownFlag(code: string): boolean {
  const c = normalizeCode(code);
  return !!c && !!FLAGS[c];
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (ch) => {
    switch (ch) {
      case "&": return "&amp;";
      case "<": return "&lt;";
      case ">": return "&gt;";
      case '"': return "&quot;";
      default: return "&#39;";
    }
  });
}
