/**
 * 数据格式化工具函数
 * 作者: TanXiang
 *
 * 提供字节、速率、数字等通用格式化纯函数
 * 所有函数均为无副作用纯函数，可在任意组件中复用
 */

/**
 * 将字节数格式化为带单位的可读字符串
 *
 * 非有限值（NaN/Infinity）与非正数（0/负数）统一返回 "0 B"，
 * 防止 Math.log 产生 NaN 或单位索引越界输出 "NaN undefined"
 *
 * @param bytes - 原始字节数
 * @param fractionDigits - 小数位数（1 用于紧凑的连接流量/测速展示，2 为默认）
 * @returns 格式化后的字符串，如 "1.50 MB"
 *
 * @example
 * formatBytes(0)        // "0 B"
 * formatBytes(-100)    // "0 B"
 * formatBytes(NaN)     // "0 B"
 * formatBytes(1536)    // "1.50 KB"
 * formatBytes(1048576) // "1.00 MB"
 * formatBytes(1536, 1) // "1.5 KB"
 */
export function formatBytes(bytes: number, fractionDigits: 1 | 2 = 2): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(
    Math.floor(Math.log(bytes) / Math.log(k)),
    sizes.length - 1
  );
  return parseFloat((bytes / Math.pow(k, i)).toFixed(fractionDigits)) + " " + sizes[i];
}

/**
 * 将实时速率（字节/秒）格式化为带单位的可读字符串
 *
 * 后端 traffic 快照的 download_speed/upload_speed 语义为字节/秒
 * （traffic_monitor.rs：Clash API connections 字节 delta / 采集周期），
 * 全站实时速率展示（首页 SpeedChart / 连接审计页）统一走本函数，
 * 按 1024 字节进制输出 KB/s / MB/s（与 formatThroughputCompact 同口径）
 *
 * 非有限值（NaN/Infinity）与非正数（0/负数）统一返回 "0 KB/s"，
 * 防止 Math.log 产生 NaN 或单位索引越界输出 "NaN undefined"
 *
 * @param bytesPerSec - 每秒字节数
 * @returns 格式化后的速率字符串，如 "1.50 MB/s"
 *
 * @example
 * formatSpeed(0)          // "0 KB/s"
 * formatSpeed(-1)         // "0 KB/s"
 * formatSpeed(NaN)        // "0 KB/s"
 * formatSpeed(1048576)   // "1.00 MB/s"
 */
export function formatSpeed(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "0 KB/s";
  const k = 1024;
  const sizes = ["B/s", "KB/s", "MB/s", "GB/s"];
  const i = Math.min(
    Math.floor(Math.log(bytesPerSec) / Math.log(k)),
    sizes.length - 1
  );
  return parseFloat((bytesPerSec / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

/**
 * 节点吞吐展示（紧凑 1 位小数，1024 进制 KB/s）
 *
 * 后端 download_bps 实际为字节/秒（throughput.rs：downloaded_bytes / elapsed），
 * 字段名 bps 是历史误称——全站消费方（useSpeedtest/useSpeedtestActions 的
 * MB/s toast）均按 1024 字节进制换算，此处保持同一口径。
 */
export function formatThroughputCompact(bytesPerSec: number): string {
  if (!Number.isFinite(bytesPerSec) || bytesPerSec <= 0) return "0 KB/s";
  const k = 1024;
  const sizes = ["B/s", "KB/s", "MB/s", "GB/s"];
  const i = Math.min(
    Math.floor(Math.log(bytesPerSec) / Math.log(k)),
    sizes.length - 1
  );
  return parseFloat((bytesPerSec / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
}

/**
 * 将字节数转换为 MB 数值（保留 1 位小数）
 *
 * @param bytes - 原始字节数
 * @returns MB 数值字符串，如 "12.5"
 *
 * @example
 * bytesToMB(13107200) // "12.5"
 */
export function bytesToMB(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0.0";
  return (bytes / (1024 * 1024)).toFixed(1);
}

/**
 * 是否运行在 macOS 上
 *
 * 不要用 `userAgent.includes("mac")`：真实 UA 里是 `Macintosh` / `darwin`，
 * 部分环境（测试用 happy-dom、部分精简 WebView UA）根本不出现 "mac" 子串，
 * 会把 macOS 误判成非 mac —— 表现为 macOS 专属 UI/提示静默消失。
 * 这里同时匹配 UA 的三种常见写法与 navigator.platform。
 *
 * @returns 是否为 macOS；无 navigator 环境（SSR / 纯 Node）返回 false
 *
 * @example
 * isMacOS()  // macOS: true，Windows/Linux: false
 */
export function isMacOS(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent || "";
  // Macintosh（macOS 桌面 Safari）、darwin（多数 WebView/精简 UA）、Mac OS
  if (/macintosh|darwin|mac os/i.test(ua)) return true;
  const platform = (navigator as { platform?: string }).platform || "";
  return /mac/i.test(platform);
}
