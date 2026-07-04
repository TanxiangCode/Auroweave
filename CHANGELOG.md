# Changelog

本文档遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 规范，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

### Added
- 项目初始化：Tauri 2.0 + Vue 3 + TypeScript 工程骨架
- Design Tokens 双主题 CSS 变量系统（深色 + 浅色独立设计）
- 全局 TypeScript 类型定义（与 Rust serde struct 字段对齐）
- 五个 Pinia Store 骨架（proxy / subscription / settings / connection / speedtest）
- WebSocket 客户端封装（指数退避自动重连）
- 四个 IPC 封装层（proxy / subscription / settings / speedtest）
- Vue Router 配置（五个主视图，全部懒加载）
- 核心 Composable：`useReducedMotion` / `useFluidWave`
- Rust 端统一 `AppError` + `ApiResponse<T>` 结构
- Rust IPC 命令骨架（proxy / subscription / settings / speedtest）
- Rust `SidecarManager` sing-box 生命周期管理器骨架
- Tauri 无边框透明窗口配置（960×640，最小 960×600）
- Dashboard 视图（能量核骨架动画 + 三张启动卡片）
- 四个视图占位（Proxies / Routing / Audit / Settings）
- 项目文档体系（README / CHANGELOG / CONTRIBUTING / TASK / plans/ / docs/）

### Changed
- `.gitignore` 覆盖完整 Rust 构建产物、sidecar 二进制、平台缓存
- `package.json` 改名为 `auroweave`，添加 lint / test 脚本
- `vite.config.ts` 添加 `@` 路径别名 + vitest 配置
- `tsconfig.json` 添加 `@` 路径别名映射
- `tauri.conf.json` 窗口尺寸统一为 960×640，产品名改为 Auroweave
- `Cargo.toml` 添加 tokio / reqwest / sysinfo / tracing / thiserror 依赖

---

## [0.1.0] — 2026-07-04

> **M0 骨架里程碑**：项目从零初始化，完成完整工程骨架。

### Added
- 初始化提交：`chore: init project scaffold (M1-骨架提交)`

---

[Unreleased]: https://github.com/auroweave/auroweave/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/auroweave/auroweave/releases/tag/v0.1.0
