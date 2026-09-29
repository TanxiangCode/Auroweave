<div align="center">

# ✦ Auroweave

**下一代极简跨平台代理客户端**

[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey)](https://github.com/TanxiangCode/gauzeweave)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.x-green)](https://vuejs.org)
[![Sing-box](https://img.shields.io/badge/sing--box-1.14.2-purple)](https://sing-box.sagernet.org)

*专为开发者与网络极客打造 — 极简至上 · 绝对透明 · 本地安全*

</div>

---

## 目录

- [项目简介](#项目简介)
- [核心理念](#核心理念)
- [功能特性](#功能特性)
- [技术栈](#技术栈)
- [快速开始](#快速开始)
- [项目结构](#项目结构)
- [开发规范](#开发规范)
- [路线图](#路线图)
- [贡献指南](#贡献指南)
- [致谢](#致谢)

---

## 项目简介

Auroweave 是一款基于 **Tauri 2.0 + Vue 3 + Sing-box** 的现代化代理客户端，专注于 Windows / macOS 双平台（未来预留移动端接入）。

它拒绝做旧时代 Clash 的「皮肤壳子」，而是从 Sing-box 底层特性出发进行原生设计，通过**全息可视化**、**智能分流**和**零上传隐私保护**，重新定义代理客户端的使用体验。

> **关于仓库名**：仓库为 `gauzeweave`，应用名为 **Auroweave**。
> 二者刻意不同：改名会一并改动数据目录、bundle identifier、Windows 服务名
> （`AuroweaveCoreService`）、命名管道与计划任务名，导致老用户升级后读不到
> 原配置、订阅与 SQLite 流量库。仓库名可改，这些标识符不能动。

---

## 核心理念

| 理念 | 描述 |
|---|---|
| **极简至上** | 主界面常驻后台，高频操作通过全局快捷搜索框完成 |
| **绝对透明** | 分流规则、DNS 解析、链路拓扑完全可视化，打破网络黑盒 |
| **本地安全** | 订阅解构与规则合并 100% 在前端本地完成，杜绝凭证上传 |

**三大视觉原则：**
- **空间感知（Spatial Awareness）**：功能模块视为视窗内的实体卡片，通过原地形变转场减少割裂感
- **能量映射（Energy Mapping）**：网络数据可视化为流体速度与光效，而非冰冷数字
- **渐进式呈现（Progressive Disclosure）**：界面默认极简，高阶功能隐匿于次级交互中

---

## 功能特性

- **中央能量核**：实时网速驱动的流体光环动画，连接状态一目了然；圆环即代理接管总开关
- **三种代理模式**：系统代理 / TUN 全接管 / 混合，自由切换且互锁联动
- **探测面（Probe Plane）**：节点延迟的唯一真相源——样本累积 + 经验分位数，
  超时阈值按节点 RTT 自适应推导，不对特定机场过拟合
- **智能测速与负载均衡**：延迟、下载、上传三档独立测速，支持批量并发调度
- **应用级防火墙分流（App-Matrix）**：为每个指定应用精确分配独立出站节点
- **语义化安全看板（Audit）**：冰冷的规则与连接自动翻译为自然语言时间流
- **本地离线订阅转换**：支持 Clash / V2ray / Sing-box / Mihomo 格式，全程无凭证上传
- **系统代理正确性**：写入前校验内核存活、字段态判定、关闭时还原用户原配置——
  不再出现「显示已开启但整机断网」或「原配置永不还原」
- **内核与应用自更新**：GUI 内一键升级 sing-box 内核与本应用本体，带签名校验
- **原生持久化**：应用级真实流量统计（SQLite），跨端交互采用 AES-GCM 动态加密
- **全平台支持**：Windows / macOS 双端原生守护进程与无边框体验

---

## 技术栈

| 层 | 技术 |
|---|---|
| **前端框架** | Vue 3 (Composition API, `<script setup>`) + TypeScript + Vite 6 |
| **样式** | Tailwind CSS 3 + 自定义 Design Tokens (CSS 变量双主题) |
| **状态管理** | Pinia |
| **路由** | Vue Router 4 |
| **动效** | CSS Transition / `conic-gradient` + rAF / View Transitions API |
| **桌面壳** | Tauri 2.0 (Rust) |
| **代理核心** | Sing-box 1.14.2 (sidecar 模式) |
| **进程审计** | Rust `sysinfo` |
| **日志** | Rust `tracing` |
| **测试** | Vitest（前端单元）+ `cargo test`（Rust 单元 / 集成） |

---

## 快速开始

### 环境要求

- Node.js 20+
- Rust 1.77+（通过 [rustup](https://rustup.rs) 安装）
- Windows：Visual Studio C++ 构建工具
- macOS：Xcode Command Line Tools

### 开发运行

```bash
# 克隆仓库
git clone https://github.com/TanxiangCode/gauzeweave.git
cd gauzeweave

# 安装前端依赖
npm install

# 下载 sing-box 内核（sidecar，仓库内 .gitignore，构建必需）
npm run download:sing-box

# 启动开发服务器（同时启动 Tauri 窗口）
npm run tauri dev
```

> ⚠️ 忘记执行 `npm run download:sing-box` 是最常见的「装完就跑不起来」原因：
> `sidecar-bin/` 不入库，缺失时内核无法启动。

### 仅运行前端（无 Tauri）

```bash
npm run dev
# 访问 http://localhost:1420
```

### 构建生产包

```bash
npm run build:installer   # 等价于 tauri build
```

产物位于 `target/release/bundle/`（本仓是 Cargo **workspace**，target 在仓库根，
不在 `src-tauri/` 下）。

> **Windows 服务模式（可选）**：需要开机自启与 TUN 提权时，
> `npm run build:svc` 会编译守护进程 `AuroDaemon.exe` 并拷入
> `src-tauri/resources/`；`npm run uninstall:svc` 卸载。

### 构建并安装到当前系统

```bash
npm run build:install                  # 打包 + 安装（自动识别 Windows / macOS / Linux）
npm run build:install -- --skip-build  # 跳过打包，只重新安装上一次产物
```

安装行为按平台自动分流：

| 平台 | 产物 | 安装方式 |
| --- | --- | --- |
| Windows | NSIS `*.exe` | `/S` 静默安装（自动弹 UAC 提权）；无 NSIS 时用 MSI `msiexec /qn` |
| macOS | `Auroweave.app` | 关闭运行中实例 → 覆盖 `/Applications/Auroweave.app` → 自动 `xattr -dr com.apple.quarantine` |
| Linux | `.deb` / `.rpm` / `.AppImage` | `apt-get install` / `dpkg` / `rpm`；无包管理器产物时 AppImage 落到 `~/.local/bin/auroweave` |

> **macOS 注意**：当前版本未经 Apple 公证，`npm run build:install` 已自动清除隔离属性；
> 若用 `npm run tauri build` 产物手动安装，首次运行需执行：
> ```bash
> xattr -cr /Applications/Auroweave.app
> ```

### 软件自更新

设置 →「高级与内置」→ 「应用版本与自动更新」可检查并一键安装新版本，
下载进度实时显示，安装完成后需手动确认重启（重启会中断代理连接与 TUN 网卡）。

> ⚠️ **自更新依赖 GitHub Release**：需为每个版本发布
> `latest.json` + 已签名的更新产物，否则应用内检查会一直报"已是最新"。

发版时必须带签名密钥构建：

```bash
export TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/auroweave.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""   # 本机密钥无密码
npx tauri build --bundles app,dmg,nsis
```

产物位于 `target/release/bundle/`，其中 `*.sig` 为更新签名，
`latest.json` 需作为 Release 附件上传到 `releases/latest/download/latest.json`
（`tauri.conf.json` 的 `plugins.updater.endpoints` 指向该地址）。

> 🔑 **私钥丢失 = 自更新永久失效**：`~/.tauri/auroweave.key` 必须备份到安全位置，
> 且**不要提交进仓库**（`tauri.conf.json` 只存公钥，可安全入库）。
> 换密钥会让所有已发布版本的签名失效。

---

## 项目结构

```
gauzeweave/
├── src/                        # 前端源码 (Vue 3 + TS)
│   ├── api/                    # IPC 封装层 + WebSocket 客户端
│   │   ├── ipc/                # Tauri invoke() 封装（禁止组件直接调用）
│   │   └── clash-ws.ts         # WebSocket 数据流订阅
│   ├── composables/            # 可复用逻辑
│   ├── components/             # 通用/业务组件
│   ├── stores/                 # Pinia 状态管理（8 个领域 store）
│   ├── styles/
│   │   └── tokens.css          # Design Tokens（深色+浅色双主题 CSS 变量）
│   ├── types/                  # 全局 TS 类型（与 Rust struct 字段对齐）
│   ├── views/                  # 七个主视图 + panels/ 子面板
│   ├── constants.ts            # 应用常量（端口、版本号、测速参数等）
│   └── router/                 # Vue Router 配置
│
├── src-tauri/                  # 后端源码 (Rust)
│   ├── src/
│   │   ├── commands/           # Tauri IPC 命令（按领域拆分）
│   │   ├── core/               # sing-box 核心：sidecar / config_builder / stats_db
│   │   ├── probe/              # 探测面：延迟采样、分位数、故障转移、节点表
│   │   ├── speedtest/          # 上下行并行测速
│   │   ├── system/             # 系统代理 / TUN / 托盘 / 开机启动
│   │   ├── core_paths.rs       # 内核二进制路径唯一真源
│   │   └── error.rs            # 统一错误类型与 ApiResponse 结构
│   └── tests/                  # Rust 集成测试（含探测面实机验证）
│
├── crates/auroweave-svc/       # Windows 守护进程 AuroDaemon（服务模式）
├── scripts/                    # 构建/发版工具链（tsx 脚本）
├── plans/                      # 各模块详细开发计划
├── docs/                       # 架构文档、调研报告
├── tests/unit/                 # 前端单元测试（Vitest）
├── Cargo.toml                  # Cargo workspace 根（target 目录在仓库根）
├── CHANGELOG.md                # 版本变更记录
└── CONTRIBUTING.md             # 贡献规范
```

---

## 开发规范

详见 [CONTRIBUTING.md](CONTRIBUTING.md)，核心要点：

- **源码标识符用英文，注释/文档用中文**
- 提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org) 英文规范
- 禁止在组件内直接调用 `invoke()`，必须通过 `src/api/ipc/` 封装层
- Rust 端禁止 `unwrap()` / `expect()` 用于可能失败的业务逻辑
- 所有装饰性动画必须接入 `useReducedMotion()`

---

## 路线图

| 里程碑 | 状态 | 描述 |
|---|---|---|
| M1 代理闭环 | ✅ 已完成 | sing-box 原生集成、多格式订阅解析、全链路隧道打通 |
| M2 视觉系统 | ✅ 已完成 | 全局 Spotlight 命令框、能量核动画、完整视觉设计落地 |
| M3 智能测速 | ✅ 已完成 | 延迟、并发与吞吐量实时独立测速引擎 |
| M4 分流与审计 | ✅ 已完成 | App-Matrix 进程路由矩阵、语义化全息安全看板 |
| M5 完整产品 | ✅ 已完成 | 九大分类设置面板、性能模式切换、打包发布 |
| M6 数据与安全 | ✅ 已完成 | 真实流量本地持久化（SQLite）、IPC 令牌 AES-GCM 加密 |
| M7–M14 智能分流 v2 | ✅ 已完成 | DNS 响应级分流、配置编辑器、多源测速（0.2.0 ~ 0.6.0） |
| 服务模式与自更新 | ✅ 已完成 | Windows 守护进程、服务态内核升级、应用本体自更新（0.7.0） |
| 探测面重构 | ✅ 已完成 | 延迟唯一真相源、经验分位数、自适应超时（0.8.0） |
| M15 拓扑画布 | 🔵 规划中 | 进阶功能：全局多跳链路可视化拖拽画布 |

> 版本演进明细见 [CHANGELOG.md](CHANGELOG.md)（0.1.0 ~ 0.8.0）。
> 版本号只有 2 处真源（`package.json` 与 `Cargo.toml` 的 `[workspace.package]`），
> 用 `npm run version:check` 校验、`npm run version:set -- <x.y.z>` 改。

---

## 贡献指南

请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解提交流程和代码规范。

---

## 致谢

- [Sing-box](https://sing-box.sagernet.org) —— 代理内核
- [Tauri](https://tauri.app) —— 桌面壳框架

© 2026 TanXiang · 仓库 [TanxiangCode/gauzeweave](https://github.com/TanxiangCode/gauzeweave)
