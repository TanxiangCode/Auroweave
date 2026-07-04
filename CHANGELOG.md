# Changelog

本文档遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 规范，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

### Added
- **M4 里程碑**：完成 App-Matrix 应用矩阵与语义化 Audit 看板 (App-Matrix & Semantic Audit Engine)
- `process.rs` Rust 进程枚举器与 `routing.rs` IPC 规则持久化服务
- `AppMatrixList.vue` 进程级出站分流绑定矩阵（搜索过滤与已自定义规则筛选）
- `OutboundSelector.vue` 统一出站选择下拉组件
- `semantic-translator.ts` 规则翻译映射器（自然语言归一解毒）
- `SemanticRuleCard.vue` 语义化安全审计时间流卡片
- `RawLogStream.vue` Sing-box 极客原始日志流查看器（日志级别高亮与自动滚动）
- `AuditView.vue` 安全审计看板主舞台（真实连接数统计 banner 与动态时间流）
- **M3 里程碑**：完成智能测速引擎与负载均衡 (Speed Test Engine & Load Balancer)
- `throughput.rs` Rust 限定时长流式 HTTP 分块下载与上传吞吐量测速引擎
- `scheduler.rs` 串行批量测速调度器（结合 `speedtest-progress` Tauri Event 实时进度推送与取消令牌）
- `speedtest.store.ts` Pinia 测速 Store，实现延迟与速率多维度缓存管理
- `NodeCard.vue` 节点卡片组件增加独立 `⚡延迟` 与 `📶测速` 响应按钮及动态 spinner 动画
- `SpeedtestView.vue` 智能测速大厅（一键批量测试、串行进度显示、预计时间与流量消耗确认 Modal）
- **M2 里程碑**：完成实时状态、Spotlight 命令框、内核自愈与能量核光效绑定 (Realtime & Interactive Engine)
- `CommandPalette.vue` Spotlight 全局快捷命令框（支持 `Ctrl+Space` 呼出、节点/模式/功能模糊搜索与键盘导航）
- `SpeedChart.vue` Canvas 实时网速历史趋势双线图（下载/上传实时平滑描绘）
- `Toast.vue` 全局消息通知组件与 `useToast` Composable
- `clash-ws.ts` WebSocket /traffic 与 /connections 自动重连与 Sing-box 数据归一化
- `connection.store.ts` EMA 指数移动平均平滑网速（`α=0.15`）与 60 点采样队列
- `FluidWave.vue` 能量核旋转频次与光晕强弱动态绑定 `smoothDownloadSpeed`
- `subscription.rs` 在保存配置前自动备份 `config.backup.json`，并支持侧载启动失败自动回滚
- **M1 里程碑**：完成代理核心闭环（Core Proxy Pipeline）
- `scripts/download-sidecar.ps1` & `scripts/download-sidecar.sh` 跨平台 sing-box 1.13.14 二进制下载提取脚本
- Rust 多格式订阅解析器（支持 Clash YAML, V2Ray Base64/URI, Singbox JSON, Hysteria2, AnyTLS）
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
