/**
 * 作者: TanXiang
 * 日期: 2026/7/31
 * 描述: 下载 sing-box 二进制文件并放置到 Tauri 项目的 sidecar-bin 目录中
 * 适用平台: Windows, macOS, Linux
 * 依赖: Node.js >= 18 (用于 fetch API)
 * 注意事项:
 *   - Windows 平台需要 PowerShell 支持 Expand-Archive 命令
 *   - macOS / Linux 平台需要 tar 命令
 *   - 脚本会根据当前操作系统与架构自动选择下载对应的 sing-box 版本
 *   - 下载完成后会将二进制文件复制到 Tauri 项目的 sidecar-bin 目录中，并赋予执行权限（非 Windows 系统）
 *   - 下载的 sing-box 版本号可在 VERSION 常量中修改
 *   - 脚本会在临时目录中下载与解压文件，完成后会清理临时文件
 */

import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { execSync } from 'node:child_process';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';

const VERSION = "1.14.2";

interface TargetConfig {
  platformName: string;
  archiveName: string;
  archiveType: 'zip' | 'tar.gz';
  targetDir: string;
  exeName: string;
  destFileName: string;
}

/**
 * 根据当前运行环境（操作系统与架构）获取下载与路径配置
 */
function getTargetConfig(): TargetConfig {
  const platform = process.platform;
  const arch = process.arch;

  // 1. Windows 环境
  if (platform === 'win32') {
    const archStr = arch === 'x64' ? 'amd64' : arch === 'arm64' ? 'arm64' : 'amd64';
    return {
      platformName: `windows-${archStr}`,
      archiveName: `sing-box-${VERSION}-windows-${archStr}.zip`,
      archiveType: 'zip',
      targetDir: path.resolve(process.cwd(), 'src-tauri/sidecar-bin/windows-x64'),
      exeName: 'sing-box.exe',
      destFileName: `sing-box-${VERSION}.exe`,
    };
  }

  // 2. macOS 环境 (根据 CPU 架构选择 amd64 或 arm64)
  if (platform === 'darwin') {
    const macArch = arch === 'arm64' ? 'arm64' : 'amd64';
    return {
      platformName: `darwin-${macArch}`,
      archiveName: `sing-box-${VERSION}-darwin-${macArch}.tar.gz`,
      archiveType: 'tar.gz',
      targetDir: path.resolve(process.cwd(), `src-tauri/sidecar-bin/macos-${macArch}`),
      exeName: 'sing-box',
      destFileName: `sing-box-${VERSION}`,
    };
  }

  // 3. Linux 环境
  if (platform === 'linux') {
    let linuxArch = 'amd64';
    let dirArch = 'x86_64';

    if (arch === 'arm64') {
      linuxArch = 'arm64';
      dirArch = 'aarch64';
    } else if (arch !== 'x64') {
      throw new Error(`Unsupported Linux architecture: ${arch}`);
    }

    return {
      platformName: `linux-${linuxArch}`,
      archiveName: `sing-box-${VERSION}-linux-${linuxArch}.tar.gz`,
      archiveType: 'tar.gz',
      targetDir: path.resolve(process.cwd(), `src-tauri/sidecar-bin/linux-${dirArch}`),
      exeName: 'sing-box',
      destFileName: `sing-box-${VERSION}`,
    };
  }

  throw new Error(`Unsupported operating system: ${platform}`);
}

/**
 * 通用文件下载函数 (基于 Node.js 原生 fetch)
 *
 * 超时语义修正（2026-09-27）：原实现用 `AbortSignal.timeout(120000)` 作为
 * **整段请求**的总时限，而 sing-box 压缩包约 80MB —— 在正常但偏慢的网络下
 * 也会稳定触发 TimeoutError，下载直接失败且无任何重试（实测升级 1.14.2 时命中）。
 *
 * 改为「空闲超时」：每收到一块数据就重置计时器，只有连续 120s 一个字节都
 * 没收到才判定为断流。这才是「超时」应有的语义——它防的是卡死，不是慢。
 * 同时支持断点续传（Range 请求），中断后重试可从已有进度继续。
 */
const IDLE_TIMEOUT_MS = 120000;

async function downloadFile(
  url: string,
  destPath: string,
  attempt = 0,
): Promise<void> {
  const MAX_ATTEMPTS = 5;
  // 已下载字节数（断点续传用）
  let downloaded = 0;
  if (attempt > 0 && fs.existsSync(destPath)) {
    downloaded = fs.statSync(destPath).size;
    if (downloaded > 0) {
      console.log(`  断点续传，从 ${(downloaded / 1024 / 1024).toFixed(1)} MB 继续`);
    }
  }

  const headers: Record<string, string> = {};
  if (downloaded > 0) {
    headers.Range = `bytes=${downloaded}-`;
  }

  console.log(
    `Downloading sing-box from ${url}${attempt > 0 ? ` (第 ${attempt + 1} 次尝试)` : ''} ...`
  );

  const controller = new AbortController();
  // 空闲计时器：每次收到数据都重置
  let idleTimer: NodeJS.Timeout | undefined;
  const armIdleTimer = () => {
    if (idleTimer) clearTimeout(idleTimer);
    idleTimer = setTimeout(() => controller.abort(), IDLE_TIMEOUT_MS);
  };
  armIdleTimer();

  try {
    const response = await fetch(url, { headers, signal: controller.signal });

    // 断点续传未被服务端支持时（返回 200 而非 206），从头重下
    if (downloaded > 0 && response.status !== 206) {
      downloaded = 0;
      if (fs.existsSync(destPath)) fs.unlinkSync(destPath);
    }

    if (!response.ok && response.status !== 206) {
      throw new Error(
        `Failed to download file: ${response.statusText} (${response.status})`
      );
    }
    if (!response.body) {
      throw new Error('Response body is empty');
    }

    const fileStream = fs.createWriteStream(destPath, {
      flags: downloaded > 0 ? "a" : "w",
    });
    // @ts-ignore Node.js fetch body 类型转换
    const webStream = Readable.fromWeb(response.body);
    webStream.on("data", () => armIdleTimer());
    await pipeline(webStream, fileStream);
    if (idleTimer) clearTimeout(idleTimer);
  } catch (err) {
    if (idleTimer) clearTimeout(idleTimer);
    if (attempt + 1 < MAX_ATTEMPTS) {
      const wait = 2000 * (attempt + 1);
      console.warn(
        `  下载中断(${(err as Error).message})，${wait / 1000}s 后重试`
      );
      await new Promise((r) => setTimeout(r, wait));
      return downloadFile(url, destPath, attempt + 1);
    }
    throw err;
  }
}

/**
 * 解压压缩包 (无需额外 npm 包，调用系统自带原生工具)
 */
function extractArchive(archivePath: string, extractDir: string, archiveType: 'zip' | 'tar.gz'): void {
  console.log('Extracting archive...');
  if (!fs.existsSync(extractDir)) {
    fs.mkdirSync(extractDir, { recursive: true });
  }

  if (archiveType === 'zip') {
    // Windows 使用 PowerShell 的 Expand-Archive
    execSync(`powershell -Command "Expand-Archive -Path '${archivePath}' -DestinationPath '${extractDir}' -Force"`, {
      stdio: 'inherit',
    });
  } else {
    // macOS / Linux 使用 tar 命令
    execSync(`tar -xzf "${archivePath}" -C "${extractDir}"`, {
      stdio: 'inherit',
    });
  }
}

/**
 * 递归查找解压出的二进制可执行文件
 */
function findFileRecursively(dir: string, targetFileName: string): string | null {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      const found = findFileRecursively(fullPath, targetFileName);
      if (found) return found;
    } else if (entry.name === targetFileName) {
      return fullPath;
    }
  }
  return null;
}

async function main() {
  try {
    const config = getTargetConfig();
    const url = `https://github.com/SagerNet/sing-box/releases/download/v${VERSION}/${config.archiveName}`;

    // 创建临时工作目录
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'sing-box-download-'));
    const tempArchivePath = path.join(tempDir, config.archiveName);
    const tempExtractPath = path.join(tempDir, 'extracted');

    // 1. 下载文件
    await downloadFile(url, tempArchivePath);

    // 2. 解压文件
    extractArchive(tempArchivePath, tempExtractPath, config.archiveType);

    // 3. 查找目标二进制文件
    const foundBinary = findFileRecursively(tempExtractPath, config.exeName);
    if (!foundBinary) {
      throw new Error(`Failed to find ${config.exeName} in extracted archive.`);
    }

    // 4. 确保目标 Sidecar 目录存在并复制文件
    if (!fs.existsSync(config.targetDir)) {
      fs.mkdirSync(config.targetDir, { recursive: true });
    }

    const destinationPath = path.join(config.targetDir, config.destFileName);
    fs.copyFileSync(foundBinary, destinationPath);

    // 5. 非 Windows 系统赋予执行权限 (chmod +x)
    if (process.platform !== 'win32') {
      fs.chmodSync(destinationPath, 0o755);
    }

    console.log(`Successfully installed sing-box to ${destinationPath}`);

    // 清理临时目录
    fs.rmSync(tempDir, { recursive: true, force: true });
  } catch (error) {
    console.error('Error downloading sing-box:', error);
    process.exit(1);
  }
}

main();
