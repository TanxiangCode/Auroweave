# 模块 C — 实时状态与快捷交互

> 状态：⏳ 待开始 | 优先级：🟠 高 | 前置：模块 B

---

## 目标

让用户能够通过全局快捷键秒级切换节点/模式，并在界面上实时看到网速与连接数变化。

**验收标准**：
- 命令框 `Ctrl+Space` 呼出时间 ≤ 100ms
- 网速曲线 60fps 无丢帧

---

## 任务清单

### C-1 WebSocket 数据流接入

**文件**：`src/stores/connection.store.ts`（升级为真实接入）

- [ ] 确认 sing-box 1.13.14 `/traffic` 推送的字段格式（参考模块 A 兼容性报告）
- [ ] 接入 `subscribeTraffic()`，将 `download_speed` / `upload_speed` 实时写入 store
- [ ] EMA 平滑算法验证（`α=0.15`，测试不同网速下的平滑效果）
- [ ] 接入 `subscribeConnections()`，更新 `connections[]` 列表

---

### C-2 Spotlight 命令框

**文件**：`src/components/command-palette/CommandPalette.vue`

**功能范围**：
- [ ] 全局快捷键呼出（`Ctrl+Space`，Windows；`Cmd+Space` 冲突时改 `Cmd+Shift+P`）
- [ ] 输入框实时搜索（节点名、分组名、操作指令）
- [ ] 搜索结果分类显示（节点 / 分组 / 操作）
- [ ] 键盘导航（↑↓ 选择，Enter 确认，Esc 关闭）
- [ ] 「分组名 + 模式关键字」一键切换（如输入「香港 自动」）

**设计要求**：
- 磨砂玻璃弹出层，从屏幕中上方出现
- 关闭时平滑淡出，不要生硬消失

**Tauri 热键集成**：
```typescript
// src/composables/useHotkey.ts
import { register } from "@tauri-apps/plugin-global-shortcut";
```

---

### C-3 全局热键 Composable

**文件**：`src/composables/useHotkey.ts`

- [ ] 封装 `@tauri-apps/plugin-global-shortcut` 的注册/注销
- [ ] 支持多个热键同时注册
- [ ] 冲突检测（注册失败时友好提示，而非静默失败）
- [ ] 组件卸载时自动注销

---

### C-4 网速曲线图

**文件**：`src/components/charts/SpeedChart.vue`

- [ ] 安装并集成 `uPlot`（约 40KB，比 echarts 轻得多）
- [ ] 实时追加数据点（不重建 chart，用 `uPlot.setData()` 增量更新）
- [ ] 双线（下载 + 上传），Y 轴自动缩放
- [ ] 颜色使用 Design Token：`--accent-cyan`（下载）/ `--accent-blue`（上传）
- [ ] 窗口不可见时暂停更新（Page Visibility API）

---

### C-5 开发者工具箱

**文件**：`src/commands/settings.rs`（升级 `settings_inject_terminal_proxy`）

- [ ] 返回跨平台终端代理变量命令（bash / zsh / fish / PowerShell 格式）
- [ ] 前端复制到剪贴板功能（`@tauri-apps/plugin-clipboard-manager`）
- [ ] 终端代理注入成功提示

---

### C-6 基础错误处理与日志落盘

**文件**：`src-tauri/src/lib.rs` + 日志配置

- [ ] 配置 `tracing_subscriber` 将日志写入文件（Tauri `app_log_dir()`）
- [ ] 日志文件按日期滚动（最多保留 7 天）
- [ ] 前端错误边界（Vue `errorCaptured` / `app.config.errorHandler`）
- [ ] IPC 错误统一 Toast 提示（`src/components/common/Toast.vue`）

---

## 关键技术细节

### 命令框性能要求

呼出时间 ≤ 100ms 的实现路径：
1. 命令框组件在应用启动时即挂载（`v-show` 控制显隐，非 `v-if` 动态创建）
2. 搜索结果计算使用 `computed`，避免每次键入时重新遍历全量数据
3. 节点列表数据在 `proxyStore` 中常驻内存，无需每次呼出时重新 IPC 请求

### EMA 平滑参数调整

```typescript
// 当前默认 α = 0.15（见 constants.ts ENERGY_EMA_ALPHA）
// 如果用户反馈网速数字跳动太慢，可适当增大到 0.25
// 如果动画抖动明显，可降到 0.08
smoothSpeed = α * rawSpeed + (1 - α) * smoothSpeed;
```

---

## 提交节点

```
feat(ui): add command palette and realtime traffic display (模块C)
```
