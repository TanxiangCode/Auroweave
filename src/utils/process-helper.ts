/**
 * 进程识别、应用指纹解析与高精度生态推导工具
 * 作者: TanXiang
 */

export interface ParsedAppInfo {
  rawName: string;
  displayName: string;
  cleanPath: string;
  category: "browser" | "developer" | "social" | "media" | "ai" | "gaming" | "office" | "system" | "network" | "design" | "tool" | "other";
  icon: string;
  isKnownApp: boolean;
}

interface AppSignature {
  pattern: RegExp;
  displayName: string;
  category: ParsedAppInfo["category"];
  icon: string;
}

const APP_SIGNATURES: AppSignature[] = [
  // 1. AI 智能与大模型应用
  { pattern: /chatgpt|openai/i, displayName: "ChatGPT", category: "ai", icon: "🤖" },
  { pattern: /claude|anthropic/i, displayName: "Claude AI", category: "ai", icon: "🧠" },
  { pattern: /cursor/i, displayName: "Cursor AI 编辑器", category: "developer", icon: "🪄" },
  { pattern: /copilot/i, displayName: "GitHub Copilot", category: "developer", icon: "🤖" },
  { pattern: /ollama|llama/i, displayName: "Ollama 本地大模型", category: "ai", icon: "🦙" },
  { pattern: /midjourney/i, displayName: "Midjourney", category: "ai", icon: "🎨" },

  // 2. 现代主流浏览器
  { pattern: /google chrome|chrome|com\.google\.chrome/i, displayName: "Google Chrome", category: "browser", icon: "🌐" },
  { pattern: /msedge|microsoft edge|edge/i, displayName: "Microsoft Edge", category: "browser", icon: "🌊" },
  { pattern: /safari|com\.apple\.safari/i, displayName: "Apple Safari", category: "browser", icon: "🧭" },
  { pattern: /firefox|mozilla/i, displayName: "Mozilla Firefox", category: "browser", icon: "🦊" },
  { pattern: /arc/i, displayName: "Arc 浏览器", category: "browser", icon: "🌈" },
  { pattern: /brave/i, displayName: "Brave 浏览器", category: "browser", icon: "🦁" },
  { pattern: /opera/i, displayName: "Opera 浏览器", category: "browser", icon: "⭕" },
  { pattern: /vivaldi/i, displayName: "Vivaldi 浏览器", category: "browser", icon: "🪐" },
  { pattern: /tor browser|tor/i, displayName: "Tor 洋葱路由浏览器", category: "browser", icon: "🧅" },

  // 3. 开发者 IDE 与代码工具
  { pattern: /visual studio code|vscode|code-insiders|^code$/i, displayName: "VS Code", category: "developer", icon: "💻" },
  { pattern: /visual studio|devenv/i, displayName: "Visual Studio", category: "developer", icon: "🔷" },
  { pattern: /xcode/i, displayName: "Apple Xcode", category: "developer", icon: "🛠️" },
  { pattern: /android studio|studio64/i, displayName: "Android Studio", category: "developer", icon: "🤖" },
  { pattern: /idea|intellij/i, displayName: "IntelliJ IDEA", category: "developer", icon: "⚡" },
  { pattern: /webstorm/i, displayName: "WebStorm", category: "developer", icon: "🌪️" },
  { pattern: /pycharm/i, displayName: "PyCharm", category: "developer", icon: "🐍" },
  { pattern: /goland/i, displayName: "GoLand", category: "developer", icon: "🐹" },
  { pattern: /rustrover/i, displayName: "RustRover", category: "developer", icon: "🦀" },
  { pattern: /clion/i, displayName: "CLion", category: "developer", icon: "🎯" },
  { pattern: /phpstorm/i, displayName: "PhpStorm", category: "developer", icon: "🐘" },
  { pattern: /sublime/i, displayName: "Sublime Text", category: "developer", icon: "📑" },
  { pattern: /neovim|nvim|vim|emacs/i, displayName: "终端代码编辑器", category: "developer", icon: "📝" },

  // 4. 终端与 CLI 运行环境
  { pattern: /iterm|iterm2/i, displayName: "iTerm2", category: "developer", icon: "📟" },
  { pattern: /terminal|com\.apple\.terminal/i, displayName: "macOS 终端", category: "developer", icon: "⬛" },
  { pattern: /warp/i, displayName: "Warp 智能终端", category: "developer", icon: "🚀" },
  { pattern: /alacritty/i, displayName: "Alacritty 终端", category: "developer", icon: "⚡" },
  { pattern: /kitty/i, displayName: "Kitty 终端", category: "developer", icon: "🐱" },
  { pattern: /powershell|pwsh/i, displayName: "PowerShell", category: "developer", icon: "🟦" },
  { pattern: /cmd\.exe/i, displayName: "Windows CMD 命令提示符", category: "developer", icon: "⬛" },
  { pattern: /zsh|bash|fish|sh$/i, displayName: "Shell 会话进程", category: "developer", icon: "🐚" },

  // 5. 开发包管理器与运行时
  { pattern: /docker|dockerd|colima|containerd|podman/i, displayName: "Docker 容器引擎", category: "developer", icon: "🐳" },
  { pattern: /node|npm|pnpm|yarn|bun|deno|npx/i, displayName: "Node.js / JS 生态", category: "developer", icon: "📦" },
  { pattern: /python|python3|pip|pipenv|conda/i, displayName: "Python 运行环境", category: "developer", icon: "🐍" },
  { pattern: /cargo|rustc|rustup/i, displayName: "Rust 构建工具链", category: "developer", icon: "🦀" },
  { pattern: /git|git-remote|ssh|curl|wget|rsync/i, displayName: "Git / CLI 传输", category: "developer", icon: "🐙" },
  { pattern: /java|javac|jvm|gradle|mvn/i, displayName: "Java / Gradle 环境", category: "developer", icon: "☕" },
  { pattern: /go$|gopls/i, displayName: "Go 语言环境", category: "developer", icon: "🐹" },
  { pattern: /postman|apifox|insomnia/i, displayName: "API 调试客户端", category: "developer", icon: "📬" },
  { pattern: /redis|mongod|mysql|postgres/i, displayName: "数据库后端引擎", category: "developer", icon: "🗄️" },

  // 6. 社交沟通与协同办公
  { pattern: /wechat|weixin|微信/i, displayName: "微信 WeChat", category: "social", icon: "💬" },
  { pattern: /qq|tim/i, displayName: "腾讯 QQ", category: "social", icon: "🐧" },
  { pattern: /telegram|telegram desktop/i, displayName: "Telegram", category: "social", icon: "✈️" },
  { pattern: /discord/i, displayName: "Discord", category: "social", icon: "👾" },
  { pattern: /feishu|lark/i, displayName: "飞书 / Lark", category: "office", icon: "🕊️" },
  { pattern: /dingtalk|钉钉/i, displayName: "钉钉 DingTalk", category: "office", icon: "📌" },
  { pattern: /slack/i, displayName: "Slack", category: "office", icon: "💼" },
  { pattern: /teams/i, displayName: "Microsoft Teams", category: "office", icon: "👥" },
  { pattern: /whatsapp/i, displayName: "WhatsApp", category: "social", icon: "📞" },
  { pattern: /line/i, displayName: "LINE", category: "social", icon: "🟢" },
  { pattern: /zoom|wemeet|tencent meeting|腾讯会议/i, displayName: "网络视频会议", category: "office", icon: "📹" },

  // 7. 影音媒体与流媒体
  { pattern: /spotify/i, displayName: "Spotify 音乐", category: "media", icon: "🎵" },
  { pattern: /neteasemusic|cloudmusic|网易云/i, displayName: "网易云音乐", category: "media", icon: "🎼" },
  { pattern: /qqmusic|qq音乐/i, displayName: "QQ 音乐", category: "media", icon: "🎶" },
  { pattern: /apple music|music\.app/i, displayName: "Apple Music", category: "media", icon: "🍎" },
  { pattern: /netflix/i, displayName: "Netflix 奈飞", category: "media", icon: "🍿" },
  { pattern: /bilibili|哔哩哔哩/i, displayName: "哔哩哔哩 B站", category: "media", icon: "📺" },
  { pattern: /youtube/i, displayName: "YouTube", category: "media", icon: "▶️" },
  { pattern: /iina|vlc|potplayer|mpv|quicktime/i, displayName: "高清播放器", category: "media", icon: "🎬" },

  // 8. 生产力笔记与云存储
  { pattern: /notion/i, displayName: "Notion", category: "office", icon: "📝" },
  { pattern: /obsidian/i, displayName: "Obsidian", category: "office", icon: "💎" },
  { pattern: /logseq/i, displayName: "Logseq", category: "office", icon: "🪵" },
  { pattern: /typora/i, displayName: "Typora Markdown", category: "office", icon: "✍️" },
  { pattern: /wps|wps office|word|excel|powerpnt/i, displayName: "Office 办公套件", category: "office", icon: "📄" },
  { pattern: /onedrive/i, displayName: "OneDrive 云盘", category: "office", icon: "☁️" },
  { pattern: /dropbox/i, displayName: "Dropbox", category: "office", icon: "📦" },
  { pattern: /baidunetdisk|阿里云盘|115/i, displayName: "网络云盘", category: "office", icon: "💾" },
  { pattern: /cloudd|bird/i, displayName: "iCloud 云同步", category: "system", icon: "☁️" },

  // 9. 设计创意工具
  { pattern: /figma/i, displayName: "Figma 设计器", category: "design", icon: "🎨" },
  { pattern: /photoshop|ps\.exe/i, displayName: "Adobe Photoshop", category: "design", icon: "🖌️" },
  { pattern: /illustrator|ai\.exe/i, displayName: "Adobe Illustrator", category: "design", icon: "📐" },
  { pattern: /sketch/i, displayName: "Sketch", category: "design", icon: "💎" },
  { pattern: /blender/i, displayName: "Blender 3D", category: "design", icon: "🧊" },

  // 10. 游戏娱乐生态
  { pattern: /steam|steamwebhelper/i, displayName: "Steam 客户端", category: "gaming", icon: "🕹️" },
  { pattern: /epicgames|epic/i, displayName: "Epic Games", category: "gaming", icon: "⚔️" },
  { pattern: /riot|leagueoflegends|valorant/i, displayName: "拳头游戏平台", category: "gaming", icon: "🛡️" },
  { pattern: /battle\.net|agent\.exe/i, displayName: "暴雪战网", category: "gaming", icon: "❄️" },
  { pattern: /genshin|mihoyo|starrail/i, displayName: "米哈游游戏", category: "gaming", icon: "🎲" },

  // 11. Auroweave 与网络代理内核
  { pattern: /auroweave|aurodaemon|sing-box|clash|mihomo|v2ray|xray/i, displayName: "Auroweave 网络核心", category: "network", icon: "🛡️" },

  // 12. 系统核心底层服务 (精确匹配，避免显示为手机图标)
  { pattern: /windowserver/i, displayName: "macOS 窗口合成引擎 (WindowServer)", category: "system", icon: "🖥️" },
  { pattern: /finder/i, displayName: "macOS 访达 (Finder)", category: "system", icon: "🗂️" },
  { pattern: /dock/i, displayName: "macOS 程序坞 (Dock)", category: "system", icon: "⚓" },
  { pattern: /systemuiserver/i, displayName: "macOS 系统菜单栏", category: "system", icon: "📊" },
  { pattern: /mds|mdworker|spotlight/i, displayName: "Spotlight 索引服务", category: "system", icon: "🔍" },
  { pattern: /launchd|init|systemd/i, displayName: "系统根服务管理器", category: "system", icon: "⚙️" },
  { pattern: /mdnsresponder/i, displayName: "Bonjour DNS 广播", category: "system", icon: "📡" },
  { pattern: /configd/i, displayName: "系统网络配置守护", category: "system", icon: "🌐" },
  { pattern: /trustd|securityd/i, displayName: "系统证书与安全子系统", category: "system", icon: "🔒" },
  { pattern: /bluetoothd/i, displayName: "蓝牙设备服务", category: "system", icon: "📶" },
  { pattern: /audio|coreaudiod/i, displayName: "核心音频系统服务", category: "system", icon: "🔊" },
  { pattern: /identityservicesd|sharingd|geod/i, displayName: "Apple 云与隔空投送服务", category: "system", icon: "🍎" },
  { pattern: /svchost|explorer\.exe/i, displayName: "Windows 系统核心宿主", category: "system", icon: "🪟" },
];

/** 域名生态反向推导特征表（当内核无进程信息时智能推导） */
const DOMAIN_ECOSYSTEM_MAP: Array<{ pattern: RegExp; displayName: string; category: ParsedAppInfo["category"]; icon: string }> = [
  { pattern: /openai|chatgpt|anthropic|claude|midjourney|huggingface|groq|cohere|gemini\.google/i, displayName: "AI 智能服务", category: "ai", icon: "🤖" },
  { pattern: /github|gitlab|npmjs|pypi|cargo|docker|stackoverflow|vercel|netlify|deno\.land/i, displayName: "开发者云平台", category: "developer", icon: "💻" },
  { pattern: /telegram|t\.me|discord|twitter|x\.com|facebook|instagram|whatsapp|reddit/i, displayName: "海外社交网络", category: "social", icon: "💬" },
  { pattern: /netflix|youtube|googlevideo|spotify|disney|hulu|twitch|primevideo|deezer/i, displayName: "全球流媒体", category: "media", icon: "🎬" },
  { pattern: /steam|valve|playstation|nintendo|epicgames|riotgames|ea\.com|battle\.net/i, displayName: "游戏平台专线", category: "gaming", icon: "🎮" },
  { pattern: /bilibili|douyin|kuaishou|iqiyi|youku|weibo|zhihu|tieba|qq\.com|weixin|wechat/i, displayName: "国内社交文娱", category: "media", icon: "🎯" },
  { pattern: /taobao|tmall|jd\.com|pinduoduo|alipay|meituan/i, displayName: "国内电商生活", category: "office", icon: "🛍️" },
  { pattern: /apple|icloud|itunes|windowsupdate|microsoft|msftconnect/i, displayName: "系统云同步/更新", category: "system", icon: "🍎" },
  { pattern: /google|gstatic|googleapis|1e100/i, displayName: "Google 核心生态", category: "browser", icon: "🔍" },
  { pattern: /cloudflare|fastly|akamai|cloudfront|azure/i, displayName: "全球 CDN 加速", category: "network", icon: "⚡" },
  { pattern: /doubleclick|google-analytics|appsflyer|adjust|umeng|telemetry|track/i, displayName: "广告/分析追踪", category: "other", icon: "🚫" },
];


/**
 * 智能解析系统进程名与可执行文件路径
 */
export function parseProcessInfo(
  processName?: string,
  processPath?: string,
  domain?: string
): ParsedAppInfo {
  const raw = (processName || "").trim();
  const rawPath = (processPath || "").trim();

  // 1. 从 processPath 提取标准应用名 (macOS .app 或 Windows .exe)
  let cleanName = raw;
  let isMacApp = false;

  if (rawPath) {
    const appMatch = rawPath.match(/\/([^\/]+)\.app\//i);
    if (appMatch && appMatch[1]) {
      cleanName = appMatch[1];
      isMacApp = true;
    } else {
      const parts = rawPath.split(/[\/\\]/);
      const filename = parts[parts.length - 1] || "";
      cleanName = filename.replace(/\.exe$/i, "");

    }
  }

  const checkStr = `${cleanName} ${raw} ${rawPath}`.toLowerCase();

  // 2. 匹配知名应用特征库
  for (const sig of APP_SIGNATURES) {
    if (sig.pattern.test(checkStr)) {
      return {
        rawName: raw || cleanName || "未知进程",
        displayName: sig.displayName,
        cleanPath: rawPath,
        category: sig.category,
        icon: sig.icon,
        isKnownApp: true,
      };
    }
  }

  // 3. 智能启发式分类推导（杜绝全部给手机图标 "📱"）
  
  // 3a. macOS 桌面 App 或 Windows 桌面程序
  if (isMacApp || rawPath.includes("Program Files") || rawPath.includes("/Applications/")) {
    const formatted = cleanName.charAt(0).toUpperCase() + cleanName.slice(1);
    return {
      rawName: raw || cleanName,
      displayName: formatted,
      cleanPath: rawPath,
      category: "tool",
      icon: "💻", // 桌面应用统一给电脑/应用图标
      isKnownApp: false,
    };
  }

  // 3b. 后台守护进程 / 系统服务（以 d 结尾如 timed, airportd, 或包含 daemon / service / helper / worker）
  if (
    cleanName.endsWith("d") ||
    /daemon|service|helper|agent|worker|handler|listener|sys|kern|task/i.test(cleanName)
  ) {
    const formatted = cleanName.charAt(0).toUpperCase() + cleanName.slice(1);
    return {
      rawName: raw || cleanName,
      displayName: formatted,
      cleanPath: rawPath,
      category: "system",
      icon: "⚙️", // 系统后台服务给予齿轮图标
      isKnownApp: false,
    };
  }

  // 3c. 命令行 / CLI 工具（位于 /usr/bin, /bin, /opt/homebrew 等）
  if (/^\/usr\/bin|^\/bin|^\/usr\/local\/bin|^\/opt\/homebrew/i.test(rawPath)) {
    return {
      rawName: raw || cleanName,
      displayName: cleanName,
      cleanPath: rawPath,
      category: "developer",
      icon: "⌨️", // 命令行工具给予键盘/终端图标
      isKnownApp: false,
    };
  }

  // 3d. 带有原始进程名的其他普通应用
  if (cleanName && cleanName !== "未知主机" && cleanName !== "unknown") {
    const formatted = cleanName.charAt(0).toUpperCase() + cleanName.slice(1);
    return {
      rawName: raw || cleanName,
      displayName: formatted,
      cleanPath: rawPath,
      category: "other",
      icon: "📦", // 普通进程给予软件包/程序图标
      isKnownApp: false,
    };
  }

  // 4. 若无进程信息，依据 domain 进行生态反向推导
  if (domain) {
    for (const eco of DOMAIN_ECOSYSTEM_MAP) {
      if (eco.pattern.test(domain)) {
        return {
          rawName: domain,
          displayName: eco.displayName,
          cleanPath: "",
          category: eco.category,
          icon: eco.icon,
          isKnownApp: true,
        };
      }
    }
  }

  return {
    rawName: "系统网络栈",
    displayName: "系统网络栈",
    cleanPath: rawPath,
    category: "system",
    icon: "🌐",
    isKnownApp: false,
  };
}

