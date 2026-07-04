# TASK.md — Auroweave 主任务追踪

> 最后更新：2026-07-04
> 各模块详细计划见 [`plans/`](plans/) 目录

---

## 当前状态总览

| 模块 | 名称 | 状态 | 里程碑 | 详细计划 |
|---|---|---|---|---|
| A | 技术预研 | 🔄 进行中 | M0 | [plan-A-research.md](plans/plan-A-research.md) |
| B | 基础骨架与代理闭环 | ✅ 已完成 | M1 | [plan-B-core-proxy.md](plans/plan-B-core-proxy.md) |
| C | 实时状态与快捷交互 | ⏳ 待开始 | M2 | [plan-C-realtime.md](plans/plan-C-realtime.md) |
| D | 规则合并与内核管理 | ⏳ 待开始 | M2 | [plan-D-rules.md](plans/plan-D-rules.md) |
| E | 智能测速与负载均衡 | ⏳ 待开始 | M3 | [plan-E-speedtest.md](plans/plan-E-speedtest.md) |
| F | Routing · App-Matrix | ⏳ 待开始 | M4 | [plan-F-routing.md](plans/plan-F-routing.md) |
| G | Audit 语义化看板 | ⏳ 待开始 | M4 | [plan-G-audit.md](plans/plan-G-audit.md) |
| H | 设置页面九大面板 | ⏳ 待开始 | M5 | [plan-H-settings.md](plans/plan-H-settings.md) |
| I | 联调、测试与打包 | ⏳ 待开始 | M5 | [plan-I-release.md](plans/plan-I-release.md) |
| J | Routing · 拓扑画布 | 🔵 低优先级 | M6 | [plan-J-topology.md](plans/plan-J-topology.md) |
| K | 视觉设计系统 | 🔄 部分完成 | M2 | [plan-K-visual.md](plans/plan-K-visual.md) |

**图例**：✅ 已完成 / 🔄 进行中 / ⏳ 待开始 / 🔵 低优先级（可选）/ ❌ 阻塞

---

## 里程碑进度

### ✅ M0 — 骨架（已完成 2026-07-04）
- Git 提交：`2368e3c` — `chore: init project scaffold`
- 交付物：工程骨架、Design Tokens、类型系统、Store、路由、Rust 命令骨架

### ✅ M1 — 代理闭环（已完成 2026-07-04）
- **目标**：导入真实机场订阅 → 成功翻墙
- **交付物**：sidecar 生命周期管理、Clash/V2ray/Singbox 订阅多格式解析器、ConfigBuilder 动态配置生成器、ClashAPI HTTP 客户端、订阅与代理节点前端交互界面、**全局控制胶囊 (ControlCapsule) 与无边框窗口控制基础层 (K-2/K-3 提前交付)**

### ⏳ M2 — 基础体验完整
- **依赖**：M1 完成后并行启动 C + D + K（高级视觉与转场）
- **目标**：命令框、实时网速、规则合并、完整视觉系统与面板转场

---

## 模块执行批次（依赖关系纠正）

> ⚠️ **批次依赖修正规则**：无边框窗口设置（`decorations: false`）与窗口控制胶囊（`ControlCapsule`）属于硬绑定关系，不得后置于视觉装饰模块，已在 M1 阶段作为基础 Chrome 交付。

```
批次 1（基础设施与核心闭环）
  ├── 模块 A：技术预研
  ├── 模块 K（子任务 K-1 K-2 K-3 K-6）：Design Tokens + 无边框窗口控制胶囊/Chrome + 微交互规范 [已完成]
  └── 模块 B：基础骨架与代理闭环（核心通路） [已完成]

批次 2（M1 完成后并行推进）
  ├── 模块 C：实时状态与快捷交互 (Spotlight 命令框 / 实时网速)
  ├── 模块 D：规则合并与内核管理 (配置备份 / 唤醒重连)
  └── 模块 K（子任务 K-4 K-5 K-7）：能量核高级光效 + 设置面板 View Transitions/FLIP 转场

批次 3（C/D/K 完成后并行）
  ├── 模块 E：智能测速与负载均衡
  ├── 模块 F：Routing · App-Matrix
  └── 模块 G：Audit 语义化看板

批次 4
  └── 模块 H：设置页面九大面板

批次 5（所有功能完成后）
  └── 模块 I：联调、测试与打包发布

批次 6（可选，v1.0 发布后）
  └── 模块 J：Routing · 拓扑画布
```

---

## 当前活跃任务

### 🔴 最高优先级

#### [A-2] sing-box 1.13.14 工具链验证
- 下载 sing-box 1.13.14 二进制
- 验证 ClashAPI 接口格式
- 验证 `loadbalance` 出站 JSON 格式

#### [B-2] sing-box 下载脚本
- `scripts/download-sidecar.ps1`（Windows）
- `scripts/download-sidecar.sh`（macOS）
- SHA256 校验

#### [B-1] Sidecar 完整实现
- 使用 `tokio::process::Command` 真实拉起子进程
- 崩溃检测 + 自动重启

---

## 提交节点规划

每个里程碑完成时触发一次 Git 提交，消息格式：

| 里程碑 | 提交消息 |
|---|---|
| M0 骨架 | ✅ `chore: init project scaffold (M1-骨架提交)` |
| M1 代理闭环 | `feat(core): complete proxy pipeline end-to-end (M1)` |
| M2 基础体验 | `feat(ui): realtime traffic, command palette, visual system (M2)` |
| M3 测速 | `feat(speedtest): complete speed test engine (M3)` |
| M4 分流审计 | `feat(routing+audit): App-Matrix and semantic audit dashboard (M4)` |
| M5 发布 | `chore(release): prepare v0.1.0 production release (M5)` |
| M6 拓扑 | `feat(routing): topology canvas for advanced multi-hop (M6)` |

中间的**阶段性提交**（不限于里程碑）也会发生，在功能子模块完成时触发，使用 Conventional Commits 格式。

---

## 技术债务记录

> 在快速推进过程中主动记录，防止遗忘

| ID | 描述 | 所属模块 | 紧急度 |
|---|---|---|---|
| TD-01 | 订阅解析器暂不支持 SingBox JSON 格式的部分字段（待完善） | B | 中 |
| TD-02 | Windows UWP 应用进程路径为空时，App-Matrix 列表中不显示（待处理） | F | 低 |
| TD-03 | 语义化翻译映射表仅覆盖最常见规则，用户自定义规则复杂时依赖兜底文案 | G | 低 |
| TD-04 | 测速服务器 `fast.com` 需解析其内部 API（非直接 HTTP 下载），实现稍复杂 | E | 低 |

---

## 文档清单

| 文件 | 用途 | 状态 |
|---|---|---|
| [README.md](README.md) | 项目主页 | ✅ 已创建 |
| [CHANGELOG.md](CHANGELOG.md) | 版本变更记录 | ✅ 已创建 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 贡献规范 | ✅ 已创建 |
| [TASK.md](TASK.md) | 本文件，主任务追踪 | ✅ 已创建 |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | 架构总览 | ✅ 已创建 |
| [plans/plan-A-research.md](plans/plan-A-research.md) | 模块 A 详细计划 | ✅ 已创建 |
| [plans/plan-B-core-proxy.md](plans/plan-B-core-proxy.md) | 模块 B 详细计划 | ✅ 已创建 |
| [plans/plan-C-realtime.md](plans/plan-C-realtime.md) | 模块 C 详细计划 | ✅ 已创建 |
| [plans/plan-D-rules.md](plans/plan-D-rules.md) | 模块 D 详细计划 | ✅ 已创建 |
| [plans/plan-E-speedtest.md](plans/plan-E-speedtest.md) | 模块 E 详细计划 | ✅ 已创建 |
| [plans/plan-F-routing.md](plans/plan-F-routing.md) | 模块 F 详细计划 | ✅ 已创建 |
| [plans/plan-G-audit.md](plans/plan-G-audit.md) | 模块 G 详细计划 | ✅ 已创建 |
| [plans/plan-H-settings.md](plans/plan-H-settings.md) | 模块 H 详细计划 | ✅ 已创建 |
| [plans/plan-I-release.md](plans/plan-I-release.md) | 模块 I 详细计划 | ✅ 已创建 |
| [plans/plan-J-topology.md](plans/plan-J-topology.md) | 模块 J 详细计划 | ✅ 已创建 |
| [plans/plan-K-visual.md](plans/plan-K-visual.md) | 模块 K 详细计划 | ✅ 已创建 |
| docs/research/权限预研报告.md | 权限预研（模块A输出） | ⏳ 待产出 |
| docs/research/sing-box兼容性说明.md | sing-box 版本兼容（模块A输出） | ⏳ 待产出 |
| docs/research/竞品体验报告.md | 竞品分析（模块A输出） | ⏳ 待产出 |
| docs/research/测速服务器方案.md | 测速方案（模块A输出） | ⏳ 待产出 |
