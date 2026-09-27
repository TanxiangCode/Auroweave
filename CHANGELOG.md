# Changelog

本文档遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 规范，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)（0.x 阶段：
`0.y.0` = 功能/架构演进，`0.y.z` = 缺陷修复与调整）。

> **关于版本历史的说明（2026-09-27 补全）**
>
> 本仓库此前**从未打过 git tag**，且 `0.1.1` 之后的 202 个提交一直没有进入 CHANGELOG，
> 导致「manifest 版本号」长期停留在 `0.1.0`，而实际交付已远超该版本。
> 本次依据 `TASK.md` 的里程碑记录（M6~M14）、`plans/` 的模块计划，以及
> 全量 265 条提交的时间聚类，重建了 `0.2.0` ~ `0.7.0` 的版本边界，
> 并按各版本最后交付日期补打了 `v0.1.0` ~ `v0.7.0` 共 7 个 tag。
>
> 需要明确的是：**`0.2.0` ~ `0.7.0` 的版本号与 tag 是按提交聚类重建的
> "应有发布点"，并非历史上真实执行过的发版**（当时无 tag 可证）。
> 若你更希望按其他粒度切分，改标题后重打 tag 即可，
> 记得同步 `npm run version:set -- <x.y.z>`。
>
> 同时把版本号从 8 处收敛为 2 处真源（`package.json` 与 `Cargo.toml`
> 的 `[workspace.package]`），其余全部自动派生，详见 `CONTRIBUTING.md`。

---

## [Unreleased]

> 尚未发布的改动将在此累积。

---

## [0.7.0] — 2026-09-27

> **代理接管正确性与内核 1.14.2**：系统性修复系统代理接管的"假成功/误关/断网"问题，
> 完成 sing-box 1.14.2 适配与内核路径收敛，并补齐一键打包安装与版本号治理工具链。

### Added

- **`npm run build:install`**：一条命令完成「打包 + 安装到当前系统」，按平台自动分流
  （Windows：NSIS `/S` 静默安装并自动 UAC 提权，无 NSIS 时退到 MSI `msiexec /qn`；
  macOS：关闭运行中实例后覆盖 `/Applications/Auroweave.app` 并清除隔离属性；
  Linux：`apt-get` / `dpkg` / `rpm`，无包管理器产物时 AppImage 落到 `~/.local/bin`）。
  支持 `--skip-build` 仅重装上一次产物。见 `scripts/build-install.ts`。
- **版本号治理工具链**：`npm run version:check` 校验 8 处版本号（`package.json` /
  `package-lock.json` ×2 / `tauri.conf.json` / 两个 `Cargo.toml` / `Cargo.lock` ×2）
  是否漂移，`npm run version:set -- <x.y.z>` 一次性写入并回读校验。
  以 `package.json` 为唯一真源。见 `scripts/version-sync.ts`。
- 节点出站开启 TCP Fast Open；`dns.cache_capacity` 提升至 4096（内核默认仅 1024）；
  嗅探器限定为 http/tls/quic/dns 四种（默认全开 11 种）。
- 新增 3 项回归测试守护内核升级与 `balance` 组优化不被回退。

### Changed

- **内核升级至 sing-box 1.14.2**（1.14.0 → 1.14.2，含 77 个修复提交）。
  与本项目直接相关的包括：策略组内 UDP 域名目的地丢失、DNS 去重未遵循查询超时、
  隐式默认 DNS/出站跳过初始化阶段、FakeIP reset 缺 bucket、损坏 cache 文件崩溃、
  HTTP sniffer 把 IP 字面量误存为域名、早期握手未透传策略组、读循环空转。
  全部为零配置变更的向后兼容修复。
- `balance` 组由 `urltest` 改为 `selector`（原与 `auto` 成员完全相同却重复发健康检查，
  每轮探测请求 1175 → 805 次，降幅 31.5%），语义调整为「地区聚合」。
- 内核日志级别 `info` → `warn`；日志写入改为长连接文件句柄，轮转检查降频至每 256 行一次。
- sing-box 二进制路径收敛为唯一真源（`core_paths::resolve_core_binary`）；
  内核版本查询附带提权状态，统一 `getSingboxVersion` 单一真源。
- 全站替换硬编码颜色为 design tokens 并修正令牌误用；Canvas 图表改用正确取色方式。

### Fixed

- **关闭代理后整机断网**：关闭代理未清理 `127.0.0.1` 代理残留，导致整机无网络出口。
- **系统代理"假成功"**：开启/关闭路径缺少回读校验，命令返回成功但系统代理实际未生效；
  接管前未快照用户原代理配置，关闭时无法还原字段。
- **误关用户自有代理**：守护自动清理前缺少代理归属判定，会关掉非本应用写入的代理。
- **TUN 接管双开窗口**：TUN 接管后未强制关闭系统代理，导致双实例同时生效。
- **节点域名解析绕过 `dns.rules`**：`route.default_domain_resolver` 在 sing-box 1.14 中
  会使出站解析固定绑定单一 DNS transport 并跳过全部规则匹配，导致 dns.rules 中的
  节点域名主备对冲链从未生效。改为每个出站显式携带 `domain_resolver`
  （节点 → bootstrap 直连 DoH，direct → local），并移除该 route 字段。
- **应用级流量统计全部记为 Unknown**：内核仅在存在 process 规则时才做进程搜索，
  App-Matrix 为空时 ClashAPI `processPath` 恒为空串。显式开启 `route.find_process`。
- **服务模式下内核在线升级完全不可用**：`bin/` 被 installer 以 SDDL 锁为只读，
  GUI 进程无写权限。新增 `apply-core` 子命令由服务提权落盘，沿用 updater 同等安全强度
  （源路径白名单 + 复制后 SHA-256 复验防 TOCTOU + 文件名自生成 + 成功后回收历史内核）。
- 下载脚本的 `AbortSignal.timeout` 是整段请求总时限，80MB 内核包在偏慢网络下必然超时；
  改为空闲超时并支持断点续传与重试。
- 内核级配置校验测试不再硬编码版本号（漏改会静默跳过校验），改为复用
  `core_paths::resolve_core_binary`，找不到内核时显式失败而非跳过。
- 修正聚合状态的多选语义、批量任务状态与测速结果展示、自定义/地区分组样式不一致。

---

## [0.6.0] — 2026-09-26

> **DNS 1.14 规范化与设置体系收敛**：按 sing-box 1.14 规范重构 DNS 链路并落地 Fake-IP 双模式，
> 设置中心完成面板拆分与样式复用，测速与节点列表交互重做。

### Added

- **Fake-IP / 真实 IP 双模式**：DnsPanel 新增模式切换器与即时持久化，默认 `198.18.0.0/15`；
  开启 `cache_file.store_fakeip` 持久化映射至 `cache.db`。
- **多源测速与统一延迟统计**：多测速源接入，延迟口径统一。
- **内核升级进度上报**：真实百分比与阶段显示在按钮上，切页/切 tab 不丢失进度。
- 节点工具栏整合，批量任务按可见节点执行；批量任务补按钮锁定、取消反馈与进度占位。
- 订阅原始数据 Base64 解码明文化，自定义分组规则即时重载。
- 新增「首页显示」设置面板（统计胶囊显隐）；补全缺失设置项并修复深链高亮。

### Changed

- **按 sing-box 1.14 规范重构 DNS 链路**：依据 `default server cannot be fakeip` 约束，
  保持 `dns.final` 指向 remote，通过 `dns.rules` 规则尾部接入 fakeip 路由分发。
- 设置中心抽离独立测速与解锁面板，复用快捷键/测速/设置面板通用样式，补齐主题令牌。
- 审计连接监控与悬停暂停交互增强，路由分流组件与公共样式优化。
- 节点列表、工具栏交互与分组及区域规则管理重做。
- 自定义真实组挂到 `proxy` 主组策略组区，并在配置重建后同步分组。
- `TaskDock` 重构为自适应泊坞栏。

### Fixed

- **内核重载后 DNS 缓存残留导致节点连通性故障**：开启 fakeip 持久化根治，
  并新增 `flush_system_dns_cache` 支持轻量安全清理 macOS 本地 DNS 缓存。
- **代理回环死锁**：节点域名对冲链注入 `disable_cache` 并增加直连兜底。
- **macOS TUN 模式 TCC 权限与 Dock 图标问题**；优化 Clash API 连接池复用。
- 解析器补充 sing-box 节点与 VLESS Reality 校验；保持生成配置引用有效。
- 暴露配置重建失败（此前静默失败）；订阅详情预览改为反映真实运行配置。
- 托盘双排网速显示稳定化；自定义分组与地区分组样式统一。

---

## [0.5.0] — 2026-09-21

> **M13/M14 智能分流 v2 与配置编辑器**：分流判定从 geosite 域名名单制升级为解析结果归属制，
> 并为高级用户手改 `config.json` 提供带校验与安全写回的内置编辑器。

### Added

- **M13 智能分流 v2（plan-P）**：DNS `evaluate`/`respond` 响应级分流——
  向国内 DNS 发查询并按答案 IP 归属（`match_response` + `geoip-cn`）决定直连或走代理，
  判断依据从"域名归属"（静态滞后）变为"解析结果归属"（动态实时）。DnsPanel 仅暴露预设开关。
- **M14 配置编辑器（plan-Q）**：
  - Q1：导出 JSON Schema 并在配置生成三路径统一注入 `$schema`，
    用户在外部编辑器（VS Code + JSON Schema 插件）即得字段级补全与校验。
  - Q2：内置编辑器，保存前双重校验（JSON 语法 + `sing-box check`），
    L1/L3 双重校验 + 安全写回 + 纯重启链，**check 不过绝不写回**。
- AdvancedPanel 新增「导出 JSON Schema」入口。
- 判据设置 UI 补全；解锁检测排序键与筛选片（补提交 v1 UI 批次遗漏文件）。

### Changed

- 上下行并行测速开关（O-6）：单节点耗时约减半，默认关闭以保精度。
- `sing-box schema` 产出 444KB Draft 2020-12 schema，含全部 14 个顶层配置段与完整 `$defs`，
  且反映当前构建实际包含的功能（比官方在线 schema 更贴合本地裁剪构建）。

### Fixed

- 吞吐测速迁至 test-core 实例，默认串行保数值、零打扰。

---

## [0.4.0] — 2026-09-08

> **M11/M12 零打扰架构与优化专项**：批量解锁检测从 selector 轮换升级为独立
> test-core 实例（N 入站/N 出站/inbound 路由钉死），用户流量全程零打扰；
> 随后以优化专项收尾三源遗留。

### Added

- **AI 服务解锁检测 v1**：判据分类器 + 检测引擎 + `unlock_history` 持久化；
  NodeCard 服务徽章、批量入口、排序筛选。
- **解锁检测 v2**：自定义分组 unlock 匹配 + 虚拟分组展示链路。
- **test-core 测试内核实例（plan-N，N-1~N-5）**：拉起短生命周期第二 sing-box 进程，
  配置 N 入站 + N 出站 + N 条 inbound 路由规则，每个被测节点分得专属本地端口，
  端口流量被规则钉死到对应节点出站——主实例、主 selector、用户流量全程零打扰，
  测试结束杀掉 test-core 一切归零。含吞吐测速迁移。
- 优化专项（plan-O，O-1~O-8）：三服务并行探测腰斩单节点耗时、429 容错、
  历史数据 90 天保留期 + 订阅删除联动。

### Changed

- 大列表虚拟化、语义转换结果缓存（每秒 2400 次正则降为指纹命中复用）、
  `/proxies` 快照缓存、测速攒批、托盘节流。
- `BatchProgressCard` 共享组件抽取；EmptyState 统一空态组件 + Toast emoji 清除。
- 单位与格式单源化（`formatBytes`）。

### Fixed

- **解锁检测批量期间用户流量被带着轮换出口**：长连接服务因 IP 跳变被踢下线，
  体感"能上网但很不稳"——由 test-core 架构根治。
- 实测修正 test-core `dns.servers` 配置导致的 FATAL；全局互斥 + 单节点端口避让。
- 节点速率 8 倍数值偏差（后端值实为字节/秒）。
- 语义缓存命中时流量计数冻结首拍；测速历史趋势表误含纯延迟行、延迟攒批非单事务。
- 全站 UI glyph 清零 + EmptyState 硬编码色 token 化。

---

## [0.3.0] — 2026-09-05

> **M7/M8/M9：sing-box 1.14.0 升级与专项审查**：升级内核并消化新能力，
> 系统性清偿 P0 功能缺口、审计修复模式矩阵缺陷、做全链路性能优化与 UI 一致性长尾。

### Added

- **sing-box 升级至 1.14.0 正式版**，启用 `store_dns` 持久化（重启后域名解析无需冷启动）。
- **协议面补齐**：TUIC / Snell / hysteria 解析、multiplex、hy2 端口跳跃 1.14 新参数透传、
  原生 AnyTLS 解析与伪节点智能过滤。
- **DNS/TUN 1.14 能力落地**：`optimistic` 缓存、查询超时、可配置 DoH、
  TUN `dns_mode` 显式化、mDNS 局域网域名解析（`neighbor_domain`）。
- **P0 缺口清偿**：
  - 多订阅同时启用（激活互斥改为聚合开关）、订阅到期与流量阔达 macOS 系统通知。
  - 开机自启真实落地（`tauri-plugin-autostart`）、全局快捷键真实注册、
    DNS/TUN 设置面板可配置化。
  - 规则集手动强制更新与缓存状态展示、测速结果历史持久化（SQLite，重启不丢）。
  - 内核 sing-box 日志文件持久化（2MB 轮转）、节点置顶收藏（星标恒排最前）。
  - **系统代理守护**：外部篡改自动恢复，期望状态仲裁防打架。
  - **连通性与出口检测**：双路径探测判定流量走向与直连泄漏。
- 规则集远程订阅化（`remote` + `initial_path`，内核自动更新）。
- 全局确认弹窗统一：9 处原生 `confirm()` 替换为玻璃风格 `ConfirmDialog`。
- 首页新增当前节点延迟、出口 IP 归属地国旗；流量图配色统一。

### Changed

- **内核拉起 1.5s → 230ms**（实测）；SQLite 启用 WAL 模式 + `settings.json` mtime 缓存。
- 订阅面板双入口收敛，修复语义漂移。
- 内置区域扩充至 56 个并增加按洲划分；主策略组增加负载均衡、分组配置编辑、
  自定义区域管理。

### Fixed

- **加固 Windows 服务安全链**，阻断本地提权到 SYSTEM 的完整攻击面。
- **引入原子写入与持久化锁基础设施**（`fs_utils`：`atomic_write` 临时文件 + rename、
  `acquire_persist_lock` 全局串行化读-改-写），杜绝崩溃时半写损坏；
  CSP 由 `null` 改为严格白名单，关闭 XSS 缓解缺口。
- **热重载假成功与测速错位**；补进程死亡检测与鉴权。
- **模式×代理组合矩阵缺陷**：DNS 模式门控、PATCH 假成功防护、双开告警。
- **TUN+rule 模式下谷歌不可访问**：添加 `prefer_ipv4` 策略。
- `rule-set` 的 `download_detour` 由 `proxy` 改为 `direct`，解决启动死循环。
- 订阅导入双路径互斥残留清除，多订阅语义全线一致。
- 拓扑画布当前节点显示错误（`selector` 应取 `now` 而非 `proxies[0]`）。
- InspectModal 25 处失效 Tailwind 类重写为项目 modal 体系；连通检测失败补 toast 反馈。

---

## [0.2.0] — 2026-08-22

> **macOS 平台化与代码结构治理**：打通 macOS 全链路（系统代理 / TUN / 数据目录 / 窗口），
> 并把四个千行级巨型组件拆分为模块化目录。

### Added

- **macOS 跨平台系统代理设置**（`sysproxy`），含 macOS TUN 提权流程优化。
- **Lucide 矢量图标体系与 `BaseIcon` 组件**，重构进程指纹与语义翻译规则。
- **原生托盘快捷菜单**、7.3pt 双排实时网速、全站 Emoji 规范化；
  AppKit 原生应用图标提取与托盘右键原生排版图标恢复。
- **独立订阅中心视图**（全屏极光），集成多源导入、流量看板与属性编辑模态框；
  后端支持 `User-Info` 流量解析、正则清洗与后台定时静默更新。
- 设置中心新增 **Sing-box 内核在线热升级**与配置项补全。
- 订阅配置三维多端查看器与节点列表联动。
- 能量核动画改为小米充电动画风格（彗星光带旋转）。

### Changed

- **巨型文件模块化拆分**（本轮最大工程债清偿）：
  `ProxiesView` 1268 行 → 主入口 180 行 + 6 组件 + 4 Hook + 1 数据文件；
  `TunPanel` 857 行、`DashboardView` 789 行、`StatsView` 669 行、
  `PrivacyPanel` 同样拆分目录化，样式全量替换为 design tokens。
- 控制胶囊提升订阅中心为一级入口，完成全站路由与快捷命令联动。
- 应用分流矩阵支持原生系统图标优先渲染；增强分流规则中心（自定义域名规则 +
  交互式拓扑画布）；重构安全审计中心（语义化流、悬停暂停、进程深度识别）。
- 首页流量图重构为经典对称上下双向镜像波形图。
- 全面接入 `BaseIcon` 矢量图标体系（设置中心各面板、节点分组徽章、测速大厅、审计页）。
- 页面切换添加 KeepAlive 缓存，避免状态丢失。

### Fixed

- **macOS 数据根目录改为用户级路径**，避免权限拒绝。
- **macOS 六大 UI 与逻辑问题**；重写红绿灯组件（渲染实际按钮、移除遮挡标题的设置按钮）。
- 修复 macOS 下 sing-box 二进制匹配失败与下载名称错误。
- **消除全部 Rust 编译器警告**；托盘主线程调度防闪退。
- 能量核旋转只转背景环、内容保持静止，消除 360 度回弹；`conic-gradient` 背景可见。
- 默认混合代理端口由 7890 改为 8890。
- `AuroDaemon.exe` 资源移至 Windows 平台专属配置，`auroweave-svc` 加跨平台条件编译守卫。
- 多订阅管理、节点排序与自定义分组；修复订阅导入后列表不刷新与重复添加。
- WebSocket 重连旧连接事件冲突；内核启动自动重连。

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

> **Auroweave v0.1.0 正式版全量交付（M0~M5）**：从零初始化工程骨架，
> 完成全链路翻墙代理、Spotlight 命令框、测速大厅、App-Matrix 进程分流与安全审计看板。

### Added
- **M0 骨架里程碑**：初始化提交 `chore: init project scaffold`。
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

[Unreleased]: https://github.com/auroweave/auroweave/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/auroweave/auroweave/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/auroweave/auroweave/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/auroweave/auroweave/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/auroweave/auroweave/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/auroweave/auroweave/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/auroweave/auroweave/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/auroweave/auroweave/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/auroweave/auroweave/releases/tag/v0.1.0
