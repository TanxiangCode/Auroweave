/**
 * 语义化规则与 DNS 日志翻译器
 * 作者: TanXiang
 */

export interface SemanticAuditRecord {
  id: string;
  timestamp: string;
  domain: string;
  outbound: string;
  ruleMatched: string;
  semanticText: string;
  icon: string;
  type: "proxied" | "direct" | "blocked";
}

/** 预置规则语义翻译字典 (按规则优先级排序) */
const SEMANTIC_RULES: Array<{
  pattern: RegExp;
  icon: string;
  text: string;
}> = [
  { pattern: /adservice|doubleclick|analytics|telemetry|track/i, icon: "🚫", text: "安全拦截 · 广告/追踪拦截" },
  { pattern: /google|gstatic|youtube/i, icon: "🚀", text: "谷歌服务 · 节点加密加速" },
  { pattern: /github|microsoft|vscode/i, icon: "💻", text: "开发者服务 · 极速通道" },
  { pattern: /telegram|twitter|x\.com|facebook|instagram/i, icon: "💬", text: "社交网络 · 专线加速" },
  { pattern: /netflix|disney|spotify/i, icon: "🎬", text: "流媒体解锁 · 高清通道" },
  { pattern: /baidu|qq|wechat|bilibili|taobao|jd\.com|163\.com|\.cn$/i, icon: "🎯", text: "绕过大陆 · 国内直连" },
];

/** 将原始连接/DNS 数据转化为语义化 Audit 记录 */
export function translateConnection(
  domain: string,
  outbound: string,
  ruleMatched: string = "default"
): Omit<SemanticAuditRecord, "id" | "timestamp"> {
  const isDirect = outbound.toLowerCase() === "direct";
  const isBlock = outbound.toLowerCase() === "block" || outbound.toLowerCase() === "reject";

  let icon = isDirect ? "🎯" : isBlock ? "🚫" : "🚀";
  let semanticText = isDirect
    ? "国内请求 · 直连穿透"
    : isBlock
    ? "恶意域名 · 已阻断"
    : "海外目标 · 节点加密传输";

  for (const item of SEMANTIC_RULES) {
    if (item.pattern.test(domain)) {
      icon = item.icon;
      // 核心修复：不能纯根据正则判定，必须结合真实出站（outbound）结果修正文案，杜绝“UI造假”
      let presetText = item.text;
      
      if (isDirect && presetText.includes("加速")) {
        // 本该走代理，但实际直连了
        semanticText = presetText.replace("加速", "穿透 (可能未代理)");
      } else if (!isDirect && !isBlock && presetText.includes("直连")) {
        // 本该直连，但实际走了代理
        semanticText = presetText.replace("直连", "被强制代理");
      } else {
        semanticText = presetText;
      }
      break;
    }
  }

  const type: SemanticAuditRecord["type"] = isDirect
    ? "direct"
    : isBlock
    ? "blocked"
    : "proxied";

  return {
    domain,
    outbound,
    ruleMatched,
    semanticText,
    icon,
    type,
  };
}
