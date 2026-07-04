# 模块 A — 技术预研计划

> 状态：🔄 进行中 | 优先级：🔴 最高（阻塞后续所有模块的平台相关决策）

---

## 目标

在正式编码前，通过实验与调研消除技术风险，产出可供后续模块直接参考的决策文档。

---

## 任务清单

### A-1 权限预研报告

**输出文件**：`docs/research/权限预研报告.md`

| 功能 | 需验证的问题 | 最小权限路径 | 降级方案 |
|---|---|---|---|
| TUN 模式 | Windows UAC 弹窗时机；macOS 网络扩展申请流程 | 待填写 | 禁用 TUN，仅系统代理 |
| App 防火墙分流 | `sysinfo` 能否拿到完整应用路径；UWP 需专门 API | 待填写 | 仅支持非 UWP 应用 |
| 全局热键 | macOS 辅助功能权限冲突检测与规避 | 待填写 | 软件内热键替代 |
| 无边框窗口 | Windows Snap Layout 是否保留；macOS 红绿灯 HIG 规范 | 待填写 | 保留系统装饰边框 |
| 二进制更新 | 更新时如何校验来源签名 | 待填写 | 手动下载更新 |

**验收标准**：每项功能的「最小权限路径」与「降级方案」列明完整。

---

### A-2 sing-box 1.25.4 工具链验证

**输出文件**：`docs/research/sing-box兼容性说明.md`

- [ ] 下载 sing-box 1.25.4 二进制（Windows x64 + macOS Universal）
- [ ] 验证 `sing-box run --config config.json` 子进程调用可行性
- [ ] 验证 ClashAPI 接口（`/proxies`、`/traffic` WebSocket 等）与 Sing-box 1.25.4 的实际响应格式
- [ ] 验证负载均衡出站（`loadbalance` 类型）的 JSON 配置格式，记录与旧版差异
- [ ] 验证 `.srs` 规则集编译工具链（`sing-box rule-set compile`）子进程调用

**验收标准**：ClashAPI 格式确认，负载均衡配置格式写入 `constants.rs`。

---

### A-3 竞品体验报告

**输出文件**：`docs/research/竞品体验报告.md`

需体验并记录：
- **Clash Verge Rev**：订阅导入流程、分流配置 UX、测速体验
- **FlClash**：跨平台体验、界面交互逻辑
- **Hiddify**：测速方案、订阅管理方案

**分析维度**：
1. 哪些功能已被竞品做得足够好（避免重复投资）
2. 哪些功能存在明显痛点（Auroweave 的差异化机会）
3. 哪些功能是 Auroweave 独有的（能量核、App-Matrix、语义化审计）

---

### A-4 测速服务器方案

**输出文件**：`docs/research/测速服务器方案.md`

- [ ] 测试 `speed.cloudflare.com/__down?bytes=10000000` 在代理节点下的可达性
- [ ] 测试 `fast.com` 作为测速终点的可行性（需解析其测速 API）
- [ ] 评估备选方案：`cachefly.cachefly.net`、自建 NGINX 测速端点
- [ ] 确定默认测速 URL 列表顺序（写入 `constants.ts`）

**验收标准**：至少 2 个可用的公共测速端点，流量消耗与耗时预估写入文档。

---

### A-5 View Transitions API 兼容性调研

**输出文件**：集成入 `docs/research/权限预研报告.md` 的「窗口动效」章节

- [ ] 在 Tauri WebView2（Windows）中测试 View Transitions API 是否可用
- [ ] 在 Tauri WKWebView（macOS）中测试 View Transitions API 是否可用
- [ ] 评估帧率与视觉一致性
- [ ] 给出结论：用 View Transitions API / 手写 FLIP / 降级为缩放+淡入

**验收标准**：明确可用性结论，写入模块 K 决策备注。

---

## 依赖关系

- **本模块阻塞**：模块 K（无边框窗口 + 转场动效）、模块 F（App 审计进程权限）
- **本模块不阻塞**：模块 B 的基础骨架（可以并行开始）

---

## 输出文件清单

```
docs/research/
├── 权限预研报告.md      ← A-1 + A-5
├── sing-box兼容性说明.md ← A-2
├── 竞品体验报告.md      ← A-3
└── 测速服务器方案.md    ← A-4
```
