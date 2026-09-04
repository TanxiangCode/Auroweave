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
 * 将比特率（bps）格式化为带单位的可读速率字符串
 *
 * 非有限值（NaN/Infinity）与非正数（0/负数）统一返回 "0 bps"，
 * 防止 Math.log 产生 NaN 或单位索引越界输出 "NaN undefined"
 *
 * @param bps - 每秒比特数
 * @returns 格式化后的速率字符串，如 "1.50 Mbps"
 *
 * @example
 * formatSpeed(0)          // "0 bps"
 * formatSpeed(-1)         // "0 bps"
 * formatSpeed(NaN)        // "0 bps"
 * formatSpeed(1000000)   // "1.00 Mbps"
 */
export function formatSpeed(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return "0 bps";
  const k = 1000;
  const sizes = ["bps", "Kbps", "Mbps", "Gbps"];
  const i = Math.min(
    Math.floor(Math.log(bps) / Math.log(k)),
    sizes.length - 1
  );
  return parseFloat((bps / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

/**
 * 节点吞吐展示（紧凑 1 位小数 Kbps）
 *
 * 测速结果 download_bps 是比特率，节点卡片此前误按 1024 字节换算成
 * "KB/s"（数值偏大 8 倍），统一为 Kbps 语义。
 */
export function formatThroughputCompact(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return "0 Kbps";
  const k = 1000;
  const sizes = ["bps", "Kbps", "Mbps", "Gbps"];
  const i = Math.min(
    Math.floor(Math.log(bps) / Math.log(k)),
    sizes.length - 1
  );
  return parseFloat((bps / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
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
