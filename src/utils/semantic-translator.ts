/**
 * 语义化规则与 DNS 日志智能翻译引擎
 * 作者: TanXiang
 */
import { parseProcessInfo, type ParsedAppInfo } from "./process-helper";

export interface SemanticAuditRecord {
  id: string;
  timestamp: string;
  process: string;
  processPath?: string;
  appIcon: string;
  appDisplayName: string;
  appCategory: ParsedAppInfo["category"];
  domain: string;
  destinationIP?: string;
  port: number;
  network?: string;
  type: "proxied" | "direct" | "blocked";
  riskLevel: "safe" | "notice" | "warning" | "danger";
  category: string;
  semanticTitle: string;
  semanticDesc: string;
  securityBadges: string[];
  outbound: string;
  chains?: string[];
  ruleMatched: string;
  rulePayload?: string;
  upload_bytes: number;
  download_bytes: number;
  upload_speed?: number;
  download_speed?: number;
  start: number;
}

/** 核心生态分类规则库 */
interface SemanticCategoryRule {
  pattern: RegExp;
  categoryName: string;
  titleTemplate: (app: string, domain: string) => string;
  descTemplate: (app: string, outbound: string, isDirect: boolean, isBlock: boolean) => string;
  badges: string[];
  risk: "safe" | "notice" | "warning" | "danger";
}

const CATEGORY_RULES: SemanticCategoryRule[] = [
  // 1. 广告与隐私追踪拦截
  {
    pattern: /adservice|doubleclick|google-analytics|telemetry|track|appsflyer|adjust|umeng|beacon|adcolony/i,
    categoryName: "安全拦截",
    titleTemplate: (app) => `${app} 广告与遥测探针`,
    descTemplate: (_app, outbound, isDirect, isBlock) =>
      isBlock
        ? "已在 DNS/路由层直接熔断阻断，阻止设备指纹与隐私数据外泄"
        : isDirect
        ? "广告请求穿透直连，建议在规则中配置 Block 策略"
        : `经 [${outbound}] 节点发送，可能存在追踪风险`,
    badges: ["隐私防护", "广告过滤", "遥测阻断"],
    risk: "warning",
  },
  // 2. AI 智能与大语言模型
  {
    pattern: /openai|chatgpt|anthropic|claude|midjourney|huggingface|groq|cohere|gemini|deepseek/i,
    categoryName: "人工智能",
    titleTemplate: (app) => `${app} · AI 智能模型交互`,
    descTemplate: (app, outbound, isDirect) =>
      isDirect
        ? `由 ${app} 发起直连请求，若处于受限区域可能引发连接重置`
        : `由 ${app} 发起，经 [${outbound}] 专线加密通道安全传输，保证 API 交互低延迟`,
    badges: ["AI 计算", "TLS 1.3", "高速通道"],
    risk: "safe",
  },
  // 3. 开发者服务与开源云
  {
    pattern: /github|gitlab|npmjs|pypi|docker|stackoverflow|vercel|netlify|deno|crates\.io|maven|golang/i,
    categoryName: "开发者服务",
    titleTemplate: (app, domain) => `${app} 访问 ${domain.split(".")[0]} 开发者云`,
    descTemplate: (app, outbound, isDirect) =>
      isDirect
        ? `${app} 发起直连拉取代码或构建依赖包`
        : `${app} 经 [${outbound}] 节点加速拉取代码/镜像，突破境外网络限速`,
    badges: ["开发者通道", "代码协同", "加速下载"],
    risk: "safe",
  },
  // 4. 全球流媒体与音视频
  {
    pattern: /netflix|youtube|googlevideo|spotify|disney|hulu|twitch|primevideo|deezer|apple\.com\/video/i,
    categoryName: "全球流媒体",
    titleTemplate: (app) => `${app} · 高清音视频流媒体`,
    descTemplate: (app, outbound, isDirect) =>
      isDirect
        ? `${app} 本地直连流媒体资源`
        : `${app} 经 [${outbound}] 出口解锁全球流媒体，保障 4K/无损音质流畅缓冲`,
    badges: ["流媒体解锁", "4K 吞吐", "防卡顿"],
    risk: "safe",
  },
  // 5. 社交网络与即时通讯
  {
    pattern: /telegram|t\.me|discord|twitter|x\.com|facebook|instagram|whatsapp|signal|reddit/i,
    categoryName: "社交通讯",
    titleTemplate: (app) => `${app} · 海外社交与通讯`,
    descTemplate: (app, outbound, isDirect) =>
      isDirect
        ? `${app} 尝试直连通讯服务器`
        : `${app} 经 [${outbound}] 建立端到端加密通信专线，保障消息实时触达与隐私防监听`,
    badges: ["端到端加密", "专线长连接", "防劫持"],
    risk: "safe",
  },
  // 6. 游戏平台与联机加速
  {
    pattern: /steam|valve|playstation|nintendo|epicgames|riotgames|ea\.com|battle\.net|xbox/i,
    categoryName: "游戏联机",
    titleTemplate: (app) => `${app} · 游戏平台与对战联机`,
    descTemplate: (app, outbound, isDirect) =>
      isDirect
        ? `${app} 直连本地游戏服`
        : `${app} 经 [${outbound}] 优化电竞路由，降低丢包与对局 RTT 抖动`,
    badges: ["游戏加速", "低抖动", "UDP 直通"],
    risk: "safe",
  },
  // 7. 国内互联网与生态直连
  {
    pattern: /baidu|qq\.com|wechat|weixin|bilibili|taobao|tmall|jd\.com|pinduoduo|alipay|meituan|zhihu|163\.com|sina|douyin|kuaishou|\.cn$/i,
    categoryName: "国内穿透",
    titleTemplate: (app, domain) => `${app} 访问国内应用 (${domain})`,
    descTemplate: (app, outbound, isDirect) =>
      isDirect
        ? `命中 geosite-cn 规则，由 ${app} 发起国内直连穿透，零中转毫秒级响应`
        : `国内目标被 [${outbound}] 代理，如非特殊需求建议调整分流策略为直连`,
    badges: ["大陆直连", "零延迟", "节省流量"],
    risk: "safe",
  },
  // 8. 局域网与私有基础设施
  {
    pattern: /localhost|127\.0\.0\.1|192\.168\.|10\.|172\.(1[6-9]|2[0-9]|3[0-1])\.|\.local$/i,
    categoryName: "局域网",
    titleTemplate: (app) => `${app} 局域网内网交互`,
    descTemplate: (app) => `${app} 发起内网私有协议通信，强制本地 Direct 直通`,
    badges: ["内网直连", "局域网通信"],
    risk: "safe",
  },
];

/**
 * 将底层网络连接转换为结构化智能语义审计记录
 */
export function translateConnection(
  conn: {
    id: string;
    destination: string;
    destinationIP?: string;
    port: number;
    outbound: string;
    rule?: string;
    rulePayload?: string;
    process?: string;
    processPath?: string;
    network?: string;
    type?: string;
    chains?: string[];
    upload_bytes?: number;
    download_bytes?: number;
    upload_speed?: number;
    download_speed?: number;
    start?: number;
  }
): SemanticAuditRecord {
  const domain = conn.destination || conn.destinationIP || "未知目标";
  const outbound = conn.outbound || "direct";
  const ruleMatched = conn.rule || "Match";
  const isDirect = outbound.toLowerCase() === "direct";
  const isBlock = outbound.toLowerCase() === "block" || outbound.toLowerCase() === "reject";

  // 1. 进程识别与友好名解析
  const appInfo = parseProcessInfo(conn.process, conn.processPath, domain);

  // 2. 默认分类与描述初始化
  let category = isDirect ? "大陆直连" : isBlock ? "安全阻断" : "境外加速";
  let semanticTitle = `${appInfo.displayName} · ${domain}`;
  let semanticDesc = isDirect
    ? `命中路由规则，由 ${appInfo.displayName} 发起国内直连`
    : isBlock
    ? "已拦截恶意或未知风险请求"
    : `由 ${appInfo.displayName} 发起，经 [${outbound}] 节点加密转发`;
  let securityBadges: string[] = isDirect ? ["国内直连"] : isBlock ? ["安全拦截"] : ["加密隧道"];
  let riskLevel: SemanticAuditRecord["riskLevel"] = isBlock ? "notice" : "safe";

  // 3. 匹配语义规则库
  for (const rule of CATEGORY_RULES) {
    if (rule.pattern.test(domain) || (conn.rulePayload && rule.pattern.test(conn.rulePayload))) {
      category = rule.categoryName;
      semanticTitle = rule.titleTemplate(appInfo.displayName, domain);
      semanticDesc = rule.descTemplate(appInfo.displayName, outbound, isDirect, isBlock);
      securityBadges = [...rule.badges];
      riskLevel = isBlock ? "notice" : rule.risk;
      break;
    }
  }

  // 4. 明文 HTTP 与特殊端口安全审计检测
  if (conn.port === 80 && !isBlock && !domain.includes("generate_204")) {
    securityBadges.push("明文 HTTP");
    if (riskLevel === "safe") riskLevel = "notice";
  } else if (conn.port === 443) {
    securityBadges.push("TLS 加密");
  }

  const type: SemanticAuditRecord["type"] = isDirect ? "direct" : isBlock ? "blocked" : "proxied";

  return {
    id: conn.id,
    timestamp: new Date(conn.start || Date.now()).toLocaleTimeString("zh-CN", { hour12: false }),
    process: conn.process || "",
    processPath: conn.processPath || "",
    appIcon: appInfo.icon,
    appDisplayName: appInfo.displayName,
    appCategory: appInfo.category,
    domain,
    destinationIP: conn.destinationIP,
    port: conn.port,
    network: conn.network || "tcp",
    type,
    riskLevel,
    category,
    semanticTitle,
    semanticDesc,
    securityBadges,
    outbound,
    chains: conn.chains || [outbound],
    ruleMatched,
    rulePayload: conn.rulePayload,
    upload_bytes: conn.upload_bytes || 0,
    download_bytes: conn.download_bytes || 0,
    upload_speed: conn.upload_speed || 0,
    download_speed: conn.download_speed || 0,
    start: conn.start || Date.now(),
  };
}
