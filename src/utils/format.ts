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
 * @param bytes - 原始字节数
 * @returns 格式化后的字符串，如 "1.50 MB"
 *
 * @example
 * formatBytes(0)        // "0 B"
 * formatBytes(1536)    // "1.50 KB"
 * formatBytes(1048576) // "1.00 MB"
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
}

/**
 * 将比特率（bps）格式化为带单位的可读速率字符串
 *
 * @param bps - 每秒比特数
 * @returns 格式化后的速率字符串，如 "1.50 Mbps"
 *
 * @example
 * formatSpeed(0)          // "0 bps"
 * formatSpeed(1000000)   // "1.00 Mbps"
 */
export function formatSpeed(bps: number): string {
  if (bps === 0) return "0 bps";
  const k = 1000;
  const sizes = ["bps", "Kbps", "Mbps", "Gbps"];
  const i = Math.floor(Math.log(bps) / Math.log(k));
  return parseFloat((bps / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
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
  return (bytes / (1024 * 1024)).toFixed(1);
}
