# 模块 F — Routing 页面 · 简单模式（App-Matrix）

> 状态：⏳ 待开始 | 优先级：🟡 中 | 前置：模块 B

---

## 目标

为每个系统进程提供独立出站节点绑定能力，面向日常使用的核心分流功能。

**验收标准**：
- 能为指定应用绑定专属出站节点，实际流量走对应节点
- 下拉菜单中「链路」与「单节点」视觉上可一眼区分

---

## 任务清单

### F-1 Routing 视图框架

**文件**：`src/views/RoutingView.vue`

- [ ] 简单/高级两个 Tab（`<TabGroup>`）
- [ ] 高级模式 Tab 默认置灰（🔒 图标），悬浮提示「在高级设置中开启拓扑图功能」
- [ ] 点击置灰 Tab 时直达 Settings → AdvancedPanel → `topology_enabled` 开关
- [ ] 开关开启后，高级模式 Tab 变为可点击（响应式）

---

### F-2 进程列表获取

**文件**：`src-tauri/src/system/` 目录

**Windows 实现**：
- [ ] 使用 `sysinfo::System::processes()` 获取进程列表
- [ ] 读取每个进程的 `exe_path()`
- [ ] UWP 应用识别：通过 Windows COM API `PackageManager::FindPackageForUser()` 获取包名
- [ ] 过滤系统进程（PID < 100、无可执行路径的跳过）

**macOS 实现**：
- [ ] 同样使用 `sysinfo` 获取进程列表
- [ ] macOS 沙箱应用的路径读取权限问题（需要辅助功能权限或仅显示路径存在的进程）

**IPC 命令**：`src-tauri/src/commands/system_audit.rs`
```rust
#[tauri::command]
pub async fn audit_get_processes() -> ApiResponse<Vec<SystemProcess>> { ... }
```

---

### F-3 AppMatrixList 组件

**文件**：`src/components/routing/AppMatrixList.vue`

**UI 结构**：
```
┌─────────────────────────────────────────────────────────┐
│ 🔍 搜索应用、进程...         [已自定义规则 12]            │
├─────────────────────────────────────────────────────────┤
│ [🟦] Cursor.exe              出站：[香港自动 ▼]          │
│ [🔴] Chrome.exe              出站：[直连 ▼]              │
│ [⚪] Code.exe                出站：[跟随默认 ▼]          │
└─────────────────────────────────────────────────────────┘
```

**组件细节**：
- [ ] 顶部搜索框（实时过滤，debounce 200ms）
- [ ] 「已自定义规则」筛选胶囊（点击后只显示已修改过的进程）
- [ ] 应用图标显示：Windows 从 exe 文件提取图标（Tauri command）；macOS 从 `.app/Contents/Resources` 读取
- [ ] 每行：应用图标 + 进程名 + exe 路径（次要文字）+ 出站选择下拉
- [ ] 虚拟列表（进程数量可能 > 100，需要 `@vueuse/core` 的 `useVirtualList`）

---

### F-4 出站选择下拉菜单

**文件**：`src/components/routing/OutboundSelector.vue`

下拉选项分组显示：

```
--- 链路（高级模式搭建的多跳链路）---
  🔗 ✨ 游戏加速链          [渐变色置顶，🔗前缀]
  🔗 流媒体解锁链

--- 代理出站 ---
  🌐 香港自动              [urltest 分组]
  📍 HK-01                [单节点]
  📍 HK-02

--- 直连 / 特殊 ---
  ➡️  直连（direct）
  🚫 拦截（block）
  ⚙️  跟随默认
```

**关键设计**：
- 链路（来自模块 J 拓扑画布）与普通节点**必须视觉区分**（🔗 图标前缀 + 分组隔离）
- 链路当前无法显示实时延迟（多跳），tooltip 说明「多跳链路，延迟为估算值」

---

### F-5 应用规则持久化

**文件**：`src-tauri/src/core/config_builder.rs`（扩展）

- [ ] 将 App-Matrix 规则存储为本地 JSON（`app-rules.json`）
- [ ] 生成 config.json 时，将应用规则转换为 sing-box 进程规则（`process_name` / `process_path`）
- [ ] 规则变更后触发 sing-box 热重载

**sing-box 进程规则格式**：
```json
{
  "route": {
    "rules": [
      {
        "process_path": "/Applications/Cursor.app/Contents/MacOS/Cursor",
        "outbound": "HK-Auto"
      }
    ]
  }
}
```

---

### F-6 应用图标提取

**文件**：`src-tauri/src/system/app_icon.rs`

- [ ] Windows：使用 `ExtractIconEx` API 提取 exe 图标，转为 PNG base64 返回前端
- [ ] macOS：从 `.app` bundle 的 `Info.plist` 找到 `CFBundleIconFile`，转为 PNG base64
- [ ] 图标缓存（避免每次重新提取）

---

## 关键权限问题

参考**模块 A 权限预研报告**：
- Windows UWP 应用的进程路径可能为空，需通过包名识别
- macOS 可能需要辅助功能权限才能读取所有进程路径
- 权限不足时的降级方案：只显示有完整路径的进程

---

## 提交节点

```
feat(routing): implement App-Matrix process-level outbound binding (模块F)
```
