# Changelog

本文档遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 规范，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

> **内核升级与性能优化**：适配 sing-box 1.14.2，并修复若干由实测暴露的功能缺陷。

### Changed

- **内核升级至 sing-box 1.14.2**（1.14.0 → 1.14.2，含 77 个修复提交）。
  与本项目直接相关的包括：策略组内 UDP 域名目的地丢失、DNS 去重未遵循查询超时、
  隐式默认 DNS/出站跳过初始化阶段、FakeIP reset 缺 bucket、损坏 cache 文件崩溃、
  HTTP sniffer 把 IP 字面量误存为域名、早期握手未透传策略组、读循环空转。
  全部为零配置变更的向后兼容修复。
- `balance` 组由 `urltest` 改为 `selector`（原与 `auto` 成员完全相同却重复发健康检查，
  每轮探测请求 1175 → 805 次，降幅 31.5%），语义调整为「地区聚合」。
- 内核日志级别 `info` → `warn`；日志写入改为长连接文件句柄，轮转检查降频至每 256 行一次。

### Fixed

- **节点域名解析绕过 `dns.rules`**：`route.default_domain_resolver` 在 sing-box 1.14 中
  会使出站解析固定绑定单一 DNS transport 并跳过全部规则匹配，导致 dns.rules 中的
  节点域名主备对冲链从未生效。改为每个出站显式携带 `domain_resolver`
  （节点 → bootstrap 直连 DoH，direct → local），并移除该 route 字段。
- **应用级流量统计全部记为 Unknown**：内核仅在存在 process 规则时才做进程搜索，
  App-Matrix 为空时 ClashAPI `processPath` 恒为空串。显式开启 `route.find_process`。
- 下载脚本的 `AbortSignal.timeout` 是整段请求总时限，80MB 内核包在偏慢网络下必然超时；
  改为空闲超时并支持断点续传与重试。
- 内核级配置校验测试不再硬编码版本号（漏改会静默跳过校验），改为复用
  `core_paths::resolve_core_binary`，找不到内核时显式失败而非跳过。

### Added

- 节点出站开启 TCP Fast Open；`dns.cache_capacity` 提升至 4096（内核默认仅 1024）；
  嗅探器限定为 http/tls/quic/dns 四种（默认全开 11 种）。
- 新增 3 项回归测试守护上述优化不被回退。

---

## [0.1.1] — 2026-07-16

> **架构与安全升级**：全面重构了真实的流量持久化追踪与跨端安全交互机制。

### Added
- **应用级流量监控**：引入 Rust 后台独立循环采集，精准记录指定进程的网络消耗，并在 `GeneralPanel` 添加了专属开关保护隐私。
- **SQLite 持久化**：引入跨平台真实数据存储机制，历史流量按小时聚合入库，避免高频 I/O。
- **真实流量可视化**：`StatsView` 大盘已接入底层 SQLite 真实数据，支持年/月/日维度动态柱状图切换，新增“近 24 小时应用程序流量消耗排行榜”。

### Changed
- **目录隔离与重命名**：核心数据文件不再散乱存放在根缓存目录，已统一归置到 `Auroweave/data/` 专属目录。其中旧版流量数据库已重命名为 `stats.dat`。
- **安全加固 (AES-GCM)**：彻底废除明文 `token.txt` 设计，Tauri 客户端与系统服务层的鉴权 Token 现已重构为采用内建静态密钥 + 动态随机数加密的二进制文件 `ipc_token.bin`。

### Fixed
- **Tokio Panic 修复**：修复了在应用初始化阶段直接调用 `tokio::spawn` 导致的 `there is no reactor running` 崩溃，替换为 Tauri 标准生命周期的 `async_runtime::spawn`。
- **编译错误修复**：将 `rusqlite` 版本降级锁定至 `0.31.0`，彻底解决了由于底层 `libsqlite3-sys` 版本更新引入 `cfg_select` 不稳定特性所导致的编译器报错 (`error[E0658]`)。
- 修复了若干 Rust `serde` 蛇形与驼峰命名引起的告警。

---

## [0.1.0] — 2026-07-04

> **Auroweave v0.1.0 正式版全量交付**：完成全链路翻墙代理、Spotlight 命令框、测速大厅、App-Matrix 进程分流与安全审计看板。

### Added
- **M5 里程碑**：完成设置页面九大面板与发布打包 (Nine Settings Panels & Production Release)
- 九大分类设置面板组件（`General`, `Subscription`, `RouteMode`, `Dns`, `Tun`, `Automation`, `Hotkey`, `Privacy`, `Advanced`）
- `data-perf-mode="reduced"` 性能模式总开关（一键取消模糊滤镜与低配 GPU 降级）
- 反向精准高亮锚定（从 Routing 页点击高亮跳转至拓扑开关）
- 前端 Vitest 单元测试集（`semantic-translator.test.ts` / `speed-formatter.test.ts`）
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
- `scripts/download-sidecar.ps1` & `scripts/download-sidecar.sh` 跨平台 sing-box 1.14.0 二进制下载提取脚本
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
