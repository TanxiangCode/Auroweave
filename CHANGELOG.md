# Changelog

本文档遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 规范，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

### Added
- **M1 里程碑**：完成代理核心闭环（Core Proxy Pipeline）
- `scripts/download-sidecar.ps1` & `scripts/download-sidecar.sh` 跨平台 sing-box 1.13.14 二进制下载提取脚本
- Rust 多格式订阅解析器（支持 Clash YAML, V2Ray Base64/URI, Singbox JSON）
- `ConfigBuilder` 动态配置生成器（地区识别、urltest 自动分组、DNS & ClashAPI 监听生成）
- `ClashApiClient` HTTP 客户端（与 127.0.0.1:9090 对接）
- `SidecarManager` sing-box 子进程生命周期异步管理器（启动/停止/唤醒重连）
- 订阅导入 UI：`SettingsView.vue` 内置多格式订阅解析导入与管理
- 代理节点 UI：`ProxiesView.vue` 支持分组切换与节点点击选择


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
