/**
 * 作者: TanXiang
 * 日期: 2026/9/27
 * 描述: 一键打包并安装到当前操作系统（Windows / macOS / Linux 自动分流）
 * 依赖: Node.js >= 18，tsx（以 npm script 方式调用）
 *
 * 流程：
 *   1. `tauri build`（可用 --skip-build 跳过，只跑安装）
 *   2. 定位产物目录（向 cargo 询问 target_directory，见 resolveBundleDir）
 *   3. 平台化安装：
 *      - Windows: NSIS `*.exe` 走 `/S` 静默（提权），无 NSIS 则退到 MSI（msiexec /qn 提权）
 *      - macOS:   关闭运行中的进程 → 覆盖替换 /Applications/Auroweave.app → 去隔离属性
 *      - Linux:   `.deb` 走 apt/dpkg、`.rpm` 走 rpm、`.AppImage` 落到 ~/.local/bin
 *
 * 用法:
 *   npm run build:install                  # 打包 + 安装
 *   npm run build:install -- --skip-build  # 跳过打包，只安装已有产物
 */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const ROOT = path.resolve(process.cwd());
const APP_NAME = "Auroweave";
const MAC_APP_DIR = "/Applications";

/**
 * 定位 tauri 产物的 bundle 目录。
 *
 * 不能写死 `src-tauri/target/release/bundle`：本仓根 Cargo.toml 是 workspace
 * （成员含 src-tauri 与 crates/auroweave-svc），cargo 会把 target 统一放在
 * **workspace 根**，即 `target/release/bundle`。写死 src-tauri/target 在
 * workspace 布局下永远找不到产物。
 *
 * 以 `cargo metadata` 的 target_directory 为准（自动覆盖 CARGO_TARGET_DIR 与
 * 自定义 .cargo/config.toml）；cargo 不可用时退回两个常见布局，并取实际存在的那个。
 */
function resolveBundleDir(): string {
  const meta = spawnSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
    encoding: "utf8",
  });
  if (meta.status === 0 && meta.stdout) {
    try {
      const target = JSON.parse(meta.stdout).target_directory as string | undefined;
      if (target) {
        return path.join(target, "release", "bundle");
      }
    } catch {
      // metadata 解析失败时走下面的兜底，不该因此中断安装
    }
  }

  const candidates = [
    path.join(ROOT, "target", "release", "bundle"),
    path.join(ROOT, "src-tauri", "target", "release", "bundle"),
  ];
  // 都不存在时返回首选（--skip-build 时由调用方给出可读报错）
  return candidates.find((p) => fs.existsSync(p)) ?? candidates[0];
}

// 用 let：buildApp() 会在打包后重新解析一次——构建前目录可能还不存在，
// 那时探测只能落到 fallback 路径。
let BUNDLE_DIR = resolveBundleDir();

const args = process.argv.slice(2);
const skipBuild = args.includes("--skip-build");

function log(msg: string) {
  console.log(`[build:install] ${msg}`);
}

function die(msg: string): never {
  console.error(`[build:install] ❌ ${msg}`);
  process.exit(1);
}

/** 执行外部命令；stdio 继承，保证 sudo 密码提示与安装日志可见 */
function run(cmd: string, cmdArgs: string[]): void {
  const res = spawnSync(cmd, cmdArgs, { stdio: "inherit" });
  if (res.error) die(`执行 ${cmd} 失败：${res.error.message}`);
  if (res.status !== 0) die(`命令退出码 ${res.status}：${cmd} ${cmdArgs.join(" ")}`);
}

/** 递归收集 bundle 目录下匹配扩展名的文件，按修改时间倒序（取最新一次构建） */
function findArtifacts(exts: string[]): string[] {
  if (!fs.existsSync(BUNDLE_DIR)) return [];
  const found: string[] = [];
  const walk = (dir: string) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      const matched = exts.some((ext) => entry.name.toLowerCase().endsWith(ext));
      if (matched) found.push(full);
      else if (entry.isDirectory()) walk(full);
    }
  };
  walk(BUNDLE_DIR);
  return found.sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);
}

function newest(files: string[], label: string): string {
  if (files.length === 0) {
    die(
      `未在 ${BUNDLE_DIR} 找到 ${label} 产物。` +
        (skipBuild ? "（当前是 --skip-build，请先执行一次 npm run build:installer）" : "")
    );
  }
  const picked = files[0];
  log(`选用产物：${path.relative(ROOT, picked)}`);
  return picked;
}

function buildApp() {
  log("开始 tauri build（release）…");
  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  run(npm, ["run", "build:installer"]);
}

function installWindows() {
  const exe = findArtifacts([".exe"]);
  // NSIS 产物名为 <App>_<version>_x64-setup.exe，优先用它
  const nsisSetup = exe.find((f) => /-setup\.exe$/i.test(path.basename(f)));
  if (nsisSetup) {
    log("Windows：静默运行 NSIS 安装器（会弹 UAC 提权）…");
    // NSIS /S 需要管理员权限写 Program Files，用 Start-Process -Verb RunAs 触发 UAC
    run("powershell", [
      "-NoProfile",
      "-Command",
      `Start-Process -FilePath '${nsisSetup}' -ArgumentList '/S' -Verb RunAs -Wait`,
    ]);
    log("安装完成：NSIS installer 已退出");
    return;
  }

  const msi = findArtifacts([".msi"]);
  if (msi.length === 0) die("既没有 NSIS 安装器也没有 MSI 产物，无法安装。");
  const msiFile = newest(msi, "MSI");
  log("Windows：静默安装 MSI（会弹 UAC 提权）…");
  run("powershell", [
    "-NoProfile",
    "-Command",
    `Start-Process -FilePath 'msiexec.exe' -ArgumentList '/i','${msiFile}','/qn','/norestart' -Verb RunAs -Wait`,
  ]);
  log("安装完成：MSI installed");
}


/** ditto 保留扩展属性与签名，比 cp -R 更适合 macOS .app 覆盖安装 */
function copyAppArgs(src: string, dest: string): string[] {
  if (fs.existsSync("/usr/bin/ditto")) return ["/usr/bin/ditto", src, dest];
  return ["cp", "-R", src, dest];
}

function installMac() {
  // tauri build 一定产出 macos/Auroweave.app，直接用它
  let appPath = path.join(BUNDLE_DIR, "macos", `${APP_NAME}.app`);
  if (!fs.existsSync(appPath)) {
    const app = findArtifacts([".app"]);
    if (app.length === 0) die(`未找到 ${APP_NAME}.app（${path.relative(ROOT, appPath)}）`);
    appPath = newest(app, "app");
  }

  const target = path.join(MAC_APP_DIR, `${APP_NAME}.app`);
  log(`macOS：安装 → ${target}`);

  // 关掉正在运行的实例，否则覆盖 .app 会失败
  spawnSync("pkill", ["-f", `${APP_NAME}.app/Contents/MacOS`], { stdio: "ignore" });

  if (fs.existsSync(target)) fs.rmSync(target, { recursive: true, force: true });
  try {
    fs.cpSync(appPath, target, { recursive: true });
  } catch (err) {
    const code = (err as NodeJS.ErrnoException).code;
    if (code !== "EACCES" && code !== "EPERM") die(`复制 .app 失败：${(err as Error).message}`);
    // /Applications 不可写（非 admin 用户）时退回 sudo
    log("当前用户无权直接写 /Applications，改用 sudo…");
    run("sudo", ["rm", "-rf", target]);
    run("sudo", copyAppArgs(appPath, target));
  }

  // 本地构建产物带隔离属性会被 Gatekeeper 直接拒绝启动
  spawnSync("xattr", ["-dr", "com.apple.quarantine", target], { stdio: "ignore" });
  log("安装完成：/Applications/Auroweave.app");
}

function installLinux() {
  const deb = findArtifacts([".deb"]);
  if (deb.length > 0) {
    const file = newest(deb, "deb");
    log("Linux：安装 deb 包…");
    if (fs.existsSync("/usr/bin/apt-get")) {
      run("sudo", ["apt-get", "install", "-y", file]);
    } else {
      run("sudo", ["dpkg", "-i", file]);
      // deb 依赖可能缺失，交给 apt -f 修
      if (fs.existsSync("/usr/bin/apt-get")) run("sudo", ["apt-get", "install", "-f", "-y"]);
    }
    log("安装完成：deb");
    return;
  }

  const rpm = findArtifacts([".rpm"]);
  if (rpm.length > 0) {
    const file = newest(rpm, "rpm");
    log("Linux：安装 rpm 包…");
    run("sudo", ["rpm", "-i", "--force", file]);
    log("安装完成：rpm");
    return;
  }

  // 无包管理器产物时退到 AppImage：用户级安装，不碰系统目录
  const appimage = findArtifacts([".appimage"]);
  if (appimage.length === 0) die("未找到 deb / rpm / AppImage 产物，无法安装。");
  const file = newest(appimage, "AppImage");
  const binDir = path.join(os.homedir(), ".local", "bin");
  const dest = path.join(binDir, APP_NAME.toLowerCase());
  log(`Linux：安装 AppImage → ${dest}`);
  fs.mkdirSync(binDir, { recursive: true });
  fs.copyFileSync(file, dest);
  fs.chmodSync(dest, 0o755);
  log("安装完成：把该目录加入 PATH 后可直接用 `auroweave` 启动");
}

function main() {
  log(`平台：${process.platform} / ${process.arch}`);
  if (!skipBuild) buildApp();

  if (process.platform === "win32") installWindows();
  else if (process.platform === "darwin") installMac();
  else if (process.platform === "linux") installLinux();
  else die(`暂不支持的平台：${process.platform}`);

  log("全部完成 🎉");
}

main();
