# 模块 M — 后续迭代需求路线图

> 状态：⏳ 待开始 | 优先级：🟠 中高（打磨与深化期）| 前置：模块 A~L 全部完成
> 拟定日期：2026-09-05
> 来源：sing-box 1.14.0 升级审查 + 功能缺口四轮盘点 + UI 一致性专项审查（18 项发现）+ 性能全链路分析（12 项发现）

---

## 背景与定位

模块 B~L 已交付完整产品闭环（订阅→分流→测速→审计→设置九面板），1.14.0 升级轮完成
协议面补齐（TUIC/Snell/hysteria/multiplex）、模式语义重构（clash_mode 单一真相源）、
P0 缺口清偿（多订阅聚合/通知/自启/热键）与两项专项审查（UI 一致性、性能）。

本模块为**打磨与深化期**路线图：不再有阻塞性缺口，全部为体验质量、长尾一致性、
可维护性与深水区能力。按投入产出比分四期，每期可独立验收、独立发布。

---

## M1 — 一致性长尾清偿（打磨冲刺，1~2 天）

> UI 一致性审查 18 项发现中已修 5 项（InspectModal/confirm/卡片/开关/Tab），
> 本期清偿剩余机械性长尾。全程无架构改动，逐项替换 + 全量 vue-tsc 验收。

### M1-1 强调色 token 化（P0-3 遗留）

**现状**：`#00f2fe` 硬编码 79 处（22 个文件）、`rgba(0,242,254,x)` 大面积、红色 4 种
（`--accent-red`/`#f43f5e`/`#ff4d4f`/`--status-danger`）、绿色 3 种。SubscriptionsView 单文件 23 处为重灾区。

- [x] 全站替换为 `var(--accent-cyan-vivid)` / `var(--status-danger)` / `var(--accent-green)` 三条线
- [x] tokens.css 补齐缺失语义变量（如 `--accent-cyan-glow` 的浅色主题值）
- [x] 顺带验收浅色主题：token 化区域在 light 主题下自动切换正确（此项即浅色主题的修复标准）

**验收**：`grep -rn "#00f2fe\|#ff4d4f\|#f43f5e" src/` 归零（渐变 stop 除外）；light 主题目检无深色残留块。

### M1-2 页面头部结构统一（P1-6）

**现状**：仅 SpeedtestView 走全局 `page-header` 规范；SubscriptionsView 自造 icon-orb 头、
RoutingView/SettingsView 自造头部、ProxiesView/AuditView/DashboardView 无页面标题。

- [x] 统一 `page-header`（图标 orb + 标题 + 副标题 + 右侧 actions）节奏
- [x] ProxiesView/AuditView 至少补齐标题与副标题说明
- [x] StatsView 副标题中英混杂文案修正（"固化历史数据实时 analysis"）

### M1-3 空态/加载态组件抽取（P1-10）

**现状**：空态四种结构（豪华卡/SvgIcon/纯文字/一行字）、错误态用 emoji ⚠️、加载 spinner 三套、
formatBytes 在 4 处组件内复制实现（精度还不一致）。

- [x] 抽 `EmptyState.vue`（图标槽 + 标题 + 描述 + 可选 CTA）：组件落地，NodeListPanel 三态（含 ⚠️ emoji 清除）与 ConnectionTable 空态接入；加载态保留各视图轻量 spinner（单一 AppSpinner 抽取收益有限，成本大于收益）
- [x] 删除 ConnectionTable/SemanticRuleCard/ConnectionDetailDrawer/NodeCard 的本地 formatBytes，统一 `import { formatBytes } from "@/utils/format"`
- [x] 速率单位统一决策：测速结果统一 Kbps（发现并修正 NodeCard 历史实现把 bps 比特率按 1024 字节换算成 KB/s 的 8 倍数值偏差），ms 前空格统一
- [x] Toast 图标 emoji（ℹ）换 BaseIcon（Info）

### M1-4 批量测速进度组件抽取（低成本项）

- [x] 抽 `BatchProgressCard.vue`，ProxiesView 与 SpeedtestView 复用（逻辑已同在 speedtestStore）
- [x] 修正前端预估常量与后端不一致：`constants.ts THROUGHPUT_TEST_DURATION_SEC=8` vs 后端批量 3+3=6s / 单节点 5+5=10s——统一为共享常量并按场景区分

**M1 验收**：vue-tsc 0 error、vitest 全过、目检 8 个视图头部/空态/加载态节奏一致。

---

## M2 — 性能深水区（P1/P2 剩余项，2~3 天）

> 已完成 Top4（启动 1.5s→230ms、无效清理跳过、过滤短路、sysinfo 短路）+ WAL + settings 缓存。
> 本期处理分析报告中置信度中、需要谨慎设计的项。

### M2-1 大列表虚拟滚动（P1-4）

**现状**：/connections WS 每秒全量快照 → 数百行连接表 + AuditView 历史 200 条无虚拟化全量 diff；
ProxiesView 300 节点一次性渲染。

- [x] AuditView 连接表引入虚拟滚动（窗口化方案：scroll 监听 + 上下占位行 + 8 行缓冲，DOM 数量与总行数解耦）
- [x] 评估结论：手写窗口化（60 行零依赖），VueUse 引入为整库依赖不划算
- [ ] ProxiesView 节点网格 >100 节点时验证是否需要（grid 虚拟化复杂，先量化再决策——保留待实测）

### M2-2 语义转换结果缓存（P1-5）

**现状**：useConnectionAudit 每秒对全部活跃连接重跑 8 条正则 + 重建对象（300 连接 ≈ 2400 regex/s + 300 对象/s GC 压力）。

- [x] `translateConnection` 结果按 `id + 字段指纹`（host/rule/process 拼接 hash）缓存于 Map
- [x] 连接关闭时清条目（Map 上限封顶防泄漏）

### M2-3 ClashAPI 客户端单例与快照合并（P2-7）

**现状**：每命令 `ClashApiClient::default()` 重建（0.1ms 级，无害但浪费）；ProxiesView 激活周期
`GET /proxies` 全量拉两次（fetchGroups + fetchGroupNodes 各拉一次 200KB-1MB JSON）。

- [x] `ClashApiClient` 单例化——实施时发现真正的成本在 `/proxies` 全量拉取而非客户端重建（reqwest::Client 内部本就是 Arc 池，重建仅包装层 ~0.1ms），故以 1s TTL 快照缓存落地，三处调用点同周期内合并为一次拉取
- [x] `proxy_get_groups` 与 `proxy_get_group_nodes` 后端合并为一次快照查询（或 1-2s TTL 缓存）
- [x] 单节点测速的 selector 查找复用同一快照

### M2-4 测速链路细节（P2-8/9 剩余）

- [x] 延迟测速的 `idx%10 sleep` 移到 acquire permit 之前（permit 临界区内睡觉占并发槽）
- [x] 批量/延迟测速的 SQLite 写改 `spawn_blocking` 或攒批（延迟 300 节点写 300 次 → 攒一次）
- [ ] 上下行并行测速作为可选模式（精度换速度，默认保持串行）——实施时评估，非本轮交付

### M2-5 托盘网速双路更新收敛（P2-10）

**现状**：后端 /traffic HTTP 长连接每秒刷 NSStatusItem + 前端 IPC 3s 节流，双路重叠。

- [x] 保留后端路（窗口隐藏时前端 IPC 断连的场景需要它），ticker 降为 3s 对齐前端节奏

**M2 验收**：AuditView 开 5 分钟无内存增长（Activity Monitor 观察）；300 节点批量延迟测速
总时长对比基线有可测量下降；cargo/vitest 全过。

---

## M3 — 深水区能力（按需排期，各 3~5 天）

> 1.14 能力清单中 Tier2/3 项与功能域盘点中高成本项。每项独立立项，先出设计再动手。

### M3-1 规则集远程订阅化（1.14 initial_path）

- 现状：.srs 由应用层 reqwest 下载缓存（已支持手动强更），内核不自动更新
- 方案：切 `type: remote` + `download_detour` → 内核后台自动更新；`initial_path` 指向现有本地缓存保证断网冷启动不阻塞
- 前置核对：1.14 的 `download_detour` 已废弃改 `http_client` 语义，按当前文档实施
- 验收：规则集变更后无应用层干预自动更新；断网冷启动不阻塞

### M3-2 DNS 进阶面板（evaluate/respond 体系）

- 现状：DNS 配置面为 双服务器 + 节点域名/geosite-cn 两条规则
- 方案：落地"先问国内 DNS、答国内 IP 则直连"的响应级分流（dns/rule.md `match_response` + rule_action `evaluate/respond`）
- 风险：语义复杂（top-level 限制、tag 引用），UI 表达困难；建议先做内核配置面支持，UI 只暴露预设开关（"智能分流 v2"）
- 验收：开启后 CN 域名直连判定准确率对比 geosite 方案有提升（抽样验证）

### M3-3 配置编辑器（JSON Schema）

- 现状：AdvancedPanel 仅备份恢复；sing-box 1.14 支持 `$schema` 字段与 `sing-box schema` 命令
- 方案：内核升级命令旁挂 `schema` 导出 → 高级用户提供带 schema 校验的 config.json 编辑入口（编辑前自动 `check`）
- 验收：手改配置保存前被校验拦截，拒载错误给出定位行号

### M3-4 订阅面板双入口收敛

- 现状：SubscriptionsView（全功能卡片流）与设置页 SubscriptionPanel（列表）双入口，交互与文案已发生漂移（本轮已统一确认弹窗，结构漂移仍在）
- 方案：SubscriptionPanel 退化为"开关 + 跳转订阅中心入口"，编辑/删除全收敛到 SubscriptionsView
- 验收：两入口无行为分歧；SubscriptionPanel 代码量显著缩减

### M3-5 i18n 多语言（视需求决定是否立项）

- 现状：GeneralPanel 有 language 下拉（v-model 存了设置）但全项目无 vue-i18n、无 locale 文件——设置完全无效
- 说明：自用产品优先级低；若立项，走 vue-i18n + 中文为 source locale，英文覆盖 8 视图 + 9 面板约 400 条文案
- **决策点**：不做则应删除 language 下拉（假设置比没有更差）

---

## M4 — 明确不做（归档）

| 项 | 排除原因 |
|---|---|
| 应用自更新（Tauri updater） | 需签名公钥 + CI 构建链；自用产品手动替换 dmg 成本更低 |
| 服务端能力（endpoint server/API service/USB-IP/netns） | 服务端/Linux 专属，桌面客户端无场景 |
| L3 转发 + bridge outbound | 路由器/网关进阶玩法，场景极窄 |
| 拖拽排序节点 | 订阅刷新节点列表漂移，手动顺序无法稳定保持（已用收藏置顶替代） |
| TLS apple engine / TLS spoof | 受众窄；spoof 需 root 且抗审查场景自用不适配 |
| Windows TLS engine | 仅 Windows，当前主打 macOS |
| package_name_regex / OpenVPN 客户端 endpoint | Android 包名 / 企业 VPN 场景，订阅生态不提供此类源 |
| 高频 proxy guard 轮询加密 | 已用期望状态机解决冲突，无需加强 |
| TopologyCanvas 交互增强（拖节点等） | 现有画布已消费真实数据 + 本轮修复 now 字段；增强属炫技无刚需 |

---

## 里程碑与排期建议

```
M1（1~2 天）→ 随下个功能批次顺手发布
M2（2~3 天）→ 独立验证发布（性能专项）
M3 各项独立立项（3~5 天/项）→ 按使用痛点触发，不预先排期
M4 归档不动
```

## 全局验收门槛（每期共用）

- `cargo test --workspace` 全过（当前 34/34 基线只增不减）
- `cargo check --workspace` 0 error 0 warning（新增代码）
- `vue-tsc --noEmit` 0 error、`vitest run` 全过（当前 6/6 基线）
- 涉及内核配置的项：真实 sing-box 1.14.0 二进制 `check` 实测 + 行为差异对照实验
- 涉及 UI 的项：dark/light 双主题目检 + `data-perf-mode` 降级检查
