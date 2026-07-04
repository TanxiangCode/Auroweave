<div align="center">

# ✦ Auroweave

**下一代极简跨平台代理客户端**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey)](https://github.com/auroweave/auroweave)
[![Tauri](https://img.shields.io/badge/Tauri-2.0-orange)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3.x-green)](https://vuejs.org)
[![Sing-box](https://img.shields.io/badge/sing--box-1.25.4-purple)](https://sing-box.sagernet.org)

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
- [许可证](#许可证)

---

## 项目简介

Auroweave 是一款基于 **Tauri 2.0 + Vue 3 + Sing-box** 的现代化代理客户端，专注于 Windows / macOS 双平台（未来预留移动端接入）。

它拒绝做旧时代 Clash 的「皮肤壳子」，而是从 Sing-box 底层特性出发进行原生设计，通过**全息可视化**、**智能分流**和**零上传隐私保护**，重新定义代理客户端的使用体验。

---

## 核心理念

| 理念 | 描述 |
|---|---|
| **极简至上** | 主界面常驻后台，95% 高频操作通过全局快捷搜索框完成 |
| **绝对透明** | 分流规则、DNS 解析、链路拓扑完全可视化，打破网络黑盒 |
| **本地安全** | 订阅解构与规则合并 100% 在前端本地完成，杜绝凭证上传 |

**三大视觉原则：**
- **空间感知（Spatial Awareness）**：功能模块视为视窗内的实体卡片，通过原地形变转场减少割裂感
- **能量映射（Energy Mapping）**：网络数据可视化为流体速度与光效，而非冰冷数字
- **渐进式呈现（Progressive Disclosure）**：界面默认极简，高阶功能隐匿于次级交互中

---

## 功能特性

### 🚀 已规划功能

- **中央能量核**：实时网速驱动的流体光环动画，连接状态一目了然
- **Spotlight 命令框**：全局快捷键呼出，秒级切换节点/模式
- **智能测速**：延迟/下载/上传三档独立测速，批量串行调度
- **应用防火墙分流（App-Matrix）**：为每个应用指定独立出站节点
- **语义化安全看板（Audit）**：冰冷规则翻译为自然语言，附真实连接统计
- **本地离线订阅转换**：支持 Clash / V2ray / Sing-box / Mihomo 格式，全程无凭证上传
- **场景自动化**：Wi-Fi SSID 变化自动切换代理策略
- **拓扑画布**（高级，可选）：拖拽连线生成多跳链路 JSON

### ✅ 当前状态（v0.1.0-dev）

> 骨架阶段已完成，进入模块 B 核心实现阶段。详见 [TASK.md](TASK.md)。

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
| **代理核心** | Sing-box 1.25.4 (sidecar 模式) |
| **进程审计** | Rust `sysinfo` |
| **日志** | Rust `tracing` |
| **测试** | Vitest (前端) + Cargo test (Rust) + Playwright (E2E) |

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
git clone https://github.com/auroweave/auroweave.git
cd auroweave

# 安装前端依赖
npm install

# 启动开发服务器（同时启动 Tauri 窗口）
npm run tauri dev
```

### 仅运行前端（无 Tauri）

```bash
npm run dev
# 访问 http://localhost:1420
```

### 构建生产包

```bash
# Windows (.msi)
npm run tauri build

# macOS (.dmg, adhoc 自签)
npm run tauri build
```

> **macOS 注意**：当前版本未经 Apple 公证，首次运行需执行：
> ```bash
> xattr -cr /Applications/Auroweave.app
> ```

---

## 项目结构

```
auroweave/
├── src/                        # 前端源码 (Vue 3 + TS)
│   ├── api/                    # IPC 封装层 + WebSocket 客户端
│   │   ├── ipc/                # Tauri invoke() 封装（禁止组件直接调用）
│   │   └── clash-ws.ts         # WebSocket 数据流订阅
│   ├── composables/            # 可复用逻辑
│   ├── components/             # 通用/业务组件
│   ├── stores/                 # Pinia 状态管理（5 个领域 store）
│   ├── styles/
│   │   └── tokens.css          # Design Tokens（深色+浅色双主题 CSS 变量）
│   ├── types/                  # 全局 TS 类型（与 Rust struct 字段对齐）
│   ├── views/                  # 五个主视图
│   ├── constants.ts            # 应用常量（端口、版本号、测速参数等）
│   └── router/                 # Vue Router 配置
│
├── src-tauri/                  # 后端源码 (Rust)
│   └── src/
│       ├── commands/           # Tauri IPC 命令（4 个领域模块）
│       ├── core/               # sing-box 核心：sidecar / config_builder
│       └── error.rs            # 统一错误类型与 ApiResponse 结构
│
├── plans/                      # 各模块详细开发计划
├── docs/                       # 架构文档、调研报告
├── tests/                      # 测试（unit / rust / e2e）
├── TASK.md                     # 主任务追踪
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
| M0 骨架 | ✅ 完成 | 项目初始化、类型系统、Store、路由、Design Tokens |
| M1 代理闭环 | 🔄 进行中 | sing-box 拉起、订阅导入、真实翻墙通路 |
| M2 视觉系统 | ⏳ 待开始 | 能量核动画、控制胶囊、完整视觉设计落地 |
| M3 测速 | ⏳ 待开始 | 延迟 + 吞吐量测速全链路 |
| M4 分流+审计 | ⏳ 待开始 | App-Matrix、语义化安全看板 |
| M5 完整产品 | ⏳ 待开始 | 九大设置面板、打包发布 |
| M6 拓扑画布 | 🔵 可选 | 高级多跳链路拖拽画布 |

---

## 贡献指南

请阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 了解提交流程和代码规范。

---

## 许可证

[MIT License](LICENSE) © 2026 TanXiang
