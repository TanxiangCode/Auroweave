/**
 * 作者: TanXiang
 * 日期: 2026/9/27
 * 描述: 版本号单一真源校验 / 同步（Windows / macOS / Linux 通用，纯 Node 实现）
 *
 * 背景：项目版本号曾散落在 8 处（3 个 manifest + 3 处 lockfile + 2 处 CHANGELOG 引用），
 * 历史上 0.1.1 发布时 manifest 仍停留在 0.1.0，直到 2026-09-27 才被发现。
 *
 * 现在**真正需要人工维护的只有 2 个文件**（其余全部自动派生）：
 *   1. `package.json`                        —— npm 生态真源
 *   2. `Cargo.toml` 的 `[workspace.package]`  —— Rust workspace 真源
 *
 * 自动派生的部分（本脚本只校验、不写入）：
 *   - `package-lock.json` ×2      由 `npm install --package-lock-only` 写
 *   - `Cargo.lock` ×2             由 `cargo metadata` 写
 *   - `src-tauri/tauri.conf.json` 已改为 `"version": "../package.json"`，
 *                                 Tauri 直接读 package.json（官方 schema 支持该写法）
 *   - 两个成员 crate 用 `version.workspace = true` 继承 workspace 版本
 *
 * 用法：
 *   `npm run version:check`           只校验，漂移则退出码 1
 *   `npm run version:set -- 0.8.0`    写入 2 处真源并回读校验
 */

import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(process.cwd());

interface VersionSite {
  /** 相对路径，仅用于报错展示 */
  file: string;
  /** 人类可读的位置名 */
  label: string;
  read: () => string;
  /** 缺席表示只读（如 CHANGELOG，发版时人工补条目） */
  write?: (v: string) => void;
}

/** JSON 文件字段的读取/写入：改写时保留原有缩进与结尾换行 */
function jsonField(file: string, pointer: string[], label: string): VersionSite {
  const abs = path.join(ROOT, file);
  const getPath = (data: Record<string, unknown>) => {
    let cur: unknown = data;
    for (const k of pointer) cur = (cur as Record<string, unknown>)?.[k];
    return cur;
  };
  return {
    file,
    label,
    read: () => {
      const cur = getPath(JSON.parse(fs.readFileSync(abs, "utf8")));
      if (typeof cur !== "string") throw new Error(`${file} 的 ${pointer.join(".")} 不是字符串`);
      return cur;
    },
    write: (v) => {
      const raw = fs.readFileSync(abs, "utf8");
      const data = JSON.parse(raw) as Record<string, unknown>;
      let cur: Record<string, unknown> = data;
      for (let i = 0; i < pointer.length - 1; i++) {
        cur = cur[pointer[i]] as Record<string, unknown>;
      }
      cur[pointer[pointer.length - 1]] = v;
      // 探测原缩进：取首个缩进行的前导空格数，缺省 2
      const indentMatch = raw.match(/\n(\s+)"/);
      const indent = indentMatch ? indentMatch[1] : "  ";
      fs.writeFileSync(abs, JSON.stringify(data, null, indent) + (raw.endsWith("\n") ? "\n" : ""));
    },
  };
}

/** Cargo.lock 中 workspace 成员的 version（按包名定位，不误伤同名依赖） */
function cargoLock(file: string, pkg: string, label: string): VersionSite {
  const abs = path.join(ROOT, file);
  const re = new RegExp(`(\\[\\[package\\]\\]\\nname = "${pkg}"\\nversion = ")([^"]+)(")`);
  return {
    file,
    label,
    read: () => {
      const m = fs.readFileSync(abs, "utf8").match(re);
      if (!m) throw new Error(`${file} 中找不到包 ${pkg}`);
      return m[2];
    },
    write: (v) => {
      const raw = fs.readFileSync(abs, "utf8");
      if (!re.test(raw)) throw new Error(`${file} 中找不到包 ${pkg}`);
      fs.writeFileSync(abs, raw.replace(re, `$1${v}$3`));
    },
  };
}

/**
 * 读取成员 crate 的有效版本号。
 * 现在它们写的是 `version.workspace = true`，值继承自 workspace 根，
 * 因此这里统一读根 `Cargo.toml` 的 `[workspace.package].version`。
 */
function inheritedCargoVersion(file: string): string {
  const raw = fs.readFileSync(path.join(ROOT, file), "utf8");
  if (!/^version\.workspace\s*=\s*true/m.test(raw)) {
    throw new Error(`${file} 未使用 version.workspace = true，版本号继承已失效`);
  }
  return workspaceVersion();
}

/** 根 Cargo.toml 的 [workspace.package].version —— Rust 侧唯一真源 */
function workspaceVersion(): string {
  const raw = fs.readFileSync(path.join(ROOT, "Cargo.toml"), "utf8");
  const m = raw.match(/^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);
  if (!m) throw new Error("Cargo.toml 的 [workspace.package] 段没有 version");
  return m[1];
}

/** CHANGELOG 中最新已发布版本号（跳过 [Unreleased]） */
function changelogVersion(): string {
  const raw = fs.readFileSync(path.join(ROOT, "CHANGELOG.md"), "utf8");
  const m = raw.match(/^## \[(\d+\.\d+\.\d+)\]/m);
  if (!m) throw new Error("CHANGELOG.md 中找不到形如 `## [x.y.z]` 的版本条目");
  return m[1];
}

/** 成员 crate 站点：只读（版本继承自 workspace 根） */
function inheritedCargo(file: string, label: string): VersionSite {
  return { file, label, read: () => inheritedCargoVersion(file) };
}

/**
 * tauri.conf.json 的版本是 `"../package.json"` 路径引用（Tauri 官方支持），
 * 这里解析出最终生效的版本号。
 */
function tauriVersion(): string {
  const conf = JSON.parse(
    fs.readFileSync(path.join(ROOT, "src-tauri/tauri.conf.json"), "utf8")
  ) as { version?: string };
  if (typeof conf.version !== "string") {
    throw new Error("src-tauri/tauri.conf.json 缺少 version 字段");
  }
  // 形如 "../package.json" → 解析该文件取 version
  if (conf.version.endsWith(".json")) {
    const target = path.join(ROOT, "src-tauri", conf.version);
    const pkg = JSON.parse(fs.readFileSync(target, "utf8")) as { version?: string };
    if (typeof pkg.version !== "string") throw new Error(`${conf.version} 里没有 version`);
    return pkg.version;
  }
  return conf.version;
}

/** 人工维护的真源：只有这 2 个 */
const SOURCES: VersionSite[] = [
  jsonField("package.json", ["version"], "package.json"),
  {
    file: "Cargo.toml",
    label: "Cargo.toml [workspace.package]",
    read: workspaceVersion,
    write: (v) => {
      const abs = path.join(ROOT, "Cargo.toml");
      const raw = fs.readFileSync(abs, "utf8");
      const re = /^(\[workspace\.package\][\s\S]*?^version\s*=\s*")[^"]+(")/m;
      if (!re.test(raw)) throw new Error("Cargo.toml 的 [workspace.package].version 替换失败");
      fs.writeFileSync(abs, raw.replace(re, `$1${v}$2`));
    },
  },
];

/** 自动派生：只校验，不写入（由 npm / cargo / Tauri 自行同步） */
const DERIVED: VersionSite[] = [
  jsonField("package-lock.json", ["version"], "package-lock.json"),
  jsonField("package-lock.json", ["packages", "", "version"], "package-lock.json (root)"),
  { file: "src-tauri/tauri.conf.json", label: "tauri.conf.json (读 package.json)", read: tauriVersion },
  inheritedCargo("src-tauri/Cargo.toml", "src-tauri/Cargo.toml (继承)"),
  inheritedCargo("crates/auroweave-svc/Cargo.toml", "auroweave-svc (继承)"),
  cargoLock("Cargo.lock", "auroweave", "Cargo.lock (auroweave)"),
  cargoLock("Cargo.lock", "AuroDaemon", "Cargo.lock (AuroDaemon)"),
];

const SEMVER = /^\d+\.\d+\.\d+$/;

function check(): number {
  console.log("[version] 真源（需人工维护，共 2 处）：");
  const pkg = SOURCES[0].read();
  const ws = SOURCES[1].read();
  if (pkg === ws) {
    console.log(`  ✅ ${"package.json".padEnd(34)} ${pkg}`);
    console.log(`  ✅ ${"Cargo.toml [workspace.package]".padEnd(34)} ${ws}`);
  } else {
    console.log(`  ❌ ${"package.json".padEnd(34)} ${pkg}`);
    console.log(`  ❌ ${"Cargo.toml [workspace.package]".padEnd(34)} ${ws}`);
    console.error(`\n[version] ❌ 两个真源不一致，npm 与 Rust 侧会产出不同版本号`);
    return 1;
  }
  const expected = pkg;

  console.log("\n[version] 派生值（自动同步，只校验）：");
  let bad = 0;
  for (const site of DERIVED) {
    const actual = site.read();
    if (actual === expected) {
      console.log(`  ✅ ${site.label.padEnd(34)} ${actual}`);
    } else {
      console.log(`  ❌ ${site.label.padEnd(34)} ${actual}  (期望 ${expected})`);
      bad++;
    }
  }

  // CHANGELOG 只在「已发布最新条目」处要求一致：纯开发中允许滞后，发版时补条目
  const log = changelogVersion();
  console.log("");
  if (log === expected) {
    console.log(`  ✅ ${"CHANGELOG 最新已发布条目".padEnd(34)} ${log}`);
  } else {
    console.log(
      `  ⚠️  ${"CHANGELOG 最新已发布条目".padEnd(34)} ${log}  (当前 ${expected})\n` +
        `     发版时在 CHANGELOG 补 ## [${expected}] 条目并打 tag`
    );
  }

  if (bad > 0) {
    console.error(
      `\n[version] ❌ ${bad} 处派生值漂移。修法：` +
        `\n     npm install --package-lock-only && cargo metadata --format-version 1 > /dev/null`
    );
    return 1;
  }
  console.log("\n[version] ✅ 全部一致");
  return 0;
}

function set(target: string): number {
  if (!SEMVER.test(target)) {
    console.error(`[version] ❌ "${target}" 不是合法的 x.y.z 版本号`);
    return 1;
  }
  for (const site of SOURCES) site.write?.(target);
  console.log(`[version] 已写入真源：${SOURCES.map((s) => s.label).join(" / ")} = ${target}`);
  console.log("[version] 请接着执行，让派生值跟上：");
  console.log("  npm install --package-lock-only");
  console.log("  cargo metadata --format-version 1 > /dev/null");
  return 0;
}

const target = process.argv[2];
process.exit(target ? set(target) : check());
