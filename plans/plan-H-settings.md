# 模块 H — 设置页面九大面板

> 状态：⏳ 待开始 | 优先级：🟡 中 | 前置：模块 B、K

---

## 目标

实现所有设置功能，包含「胶囊变形转场」动效和九大分类面板。

**验收标准**：
- 九大面板功能完整，高频项在主界面，低频项在二级面板
- 「性能模式」总开关一键关闭所有装饰性动效

---

## 任务清单

### H-1 设置视图与转场动效

**文件**：`src/views/SettingsView.vue`

**「原位旋出」形变交互**：
- 点击 ⚙️ 控制胶囊，胶囊向左下方几何级放大，背景主舱雾化退至 Z 轴深处
- ✕ 按钮在设置态下变为「🔙 返回主舱」

**技术路径**（按优先级）：
1. **View Transitions API**（参考模块 A-5 预研结论）
2. **手写 FLIP 动画**（`useMorphTransition.ts`）
3. **降级**：缩放 + 交叉淡入（视觉效果打折扣，但兼容性有保障）

> ⚠️ 如果预研发现实现成本过高，直接使用降级方案，不为此转场拖慢整体进度。

- [ ] 确认转场技术方案（等模块 A-5 结论）
- [ ] 实现转场 Composable（`src/composables/useMorphTransition.ts`）
- [ ] 设置面板卡片式分组（全息发光图标 + 截断渐隐边框）

---

### H-2 面板导航

- [ ] 左侧图标导航栏（9 个分类图标，点击切换右侧面板）
- [ ] 从 Routing 页「高级模式置灰」跳转时，自动打开 AdvancedPanel 并触发三次流光高亮动效

---

### H-3 各面板实现

#### GeneralPanel.vue — 通用设置
- [ ] 外观主题：浅色 / 深色 / 跟随系统（`settings.theme`）
- [ ] 应用语言：简体中文 / English（`settings.language`）
- [ ] 开机自启（`settings.auto_start`，调用 Tauri autostart 插件）
- [ ] 最小化到托盘（`minimize_to_tray`）

#### SubscriptionPanel.vue — 订阅管理
- [ ] 订阅列表（名称 / 节点数 / 到期时间 / 更新时间）
- [ ] 添加订阅（URL + 名称输入）
- [ ] 刷新 / 删除操作
- [ ] 「自动整理分组」开关（`settings.auto_group_on_import`）
- [ ] 自动更新间隔选择（每日 / 每周 / 手动）

#### RouteModePanel.vue — 代理模式
- [ ] 代理模式切换：全局 / 规则 / 直连（`settings.proxy_mode`）
- [ ] TUN 模式开关（`settings.tun_enabled`，需权限申请流程）
- [ ] 系统代理端口配置

#### DnsPanel.vue — DNS 配置
- [ ] 远端 DNS 服务器（默认 `1.1.1.1`，支持 DoT/DoH）
- [ ] 本地 DNS 服务器（默认 `223.5.5.5`）
- [ ] 「禁用 DNS 泄露」开关

#### TunPanel.vue — TUN 模式
- [ ] TUN 接口名称
- [ ] MTU 配置
- [ ] DNS 劫持开关
- [ ] 权限申请引导流程（首次开启时显示 UAC 说明）

#### AutomationPanel.vue — 场景自动化
- [ ] 自动化规则列表（来自模块 G）
- [ ] 添加规则（触发条件 + 执行动作）
- [ ] 规则开关（不删除，仅禁用）

#### HotkeyPanel.vue — 全局热键
- [ ] 命令框热键自定义（`settings.command_palette_hotkey`）
- [ ] 热键录制（监听键盘输入，`Ctrl+Space` 格式化显示）
- [ ] 冲突检测（与系统热键冲突时警告）

#### PrivacyPanel.vue — 隐私与日志
- [ ] 日志保留天数（`log_retention_days`，默认 7 天）
- [ ] 「一键导出诊断日志」按钮（调用 `settings_export_diagnostic_log`）
- [ ] 「清除所有数据」危险操作（带二次确认）

#### AdvancedPanel.vue — 高级设置
- [ ] **「启用拓扑图功能」开关**（`settings.topology_enabled`，控制 Routing 高级模式 Tab）
- [ ] **「性能模式」总开关**（`settings.performance_mode`，关闭 blur/粒子/流体动画）
  - 开启后向 `<html>` 注入 `data-perf-mode="reduced"`
- [ ] 测速服务器 URL 列表（可编辑列表，默认见 `constants.ts`）
- [ ] sing-box 内核版本管理（当前版本 / 检查更新按钮）
- [ ] 「恢复上一次配置」入口（来自模块 D-4 备份）
- [ ] 「重置所有设置」危险操作

---

## 「性能模式」实现细节

```typescript
// settings.store.ts（已实现骨架）
function applyPerformanceMode(enabled: boolean) {
  if (enabled) {
    document.documentElement.setAttribute("data-perf-mode", "reduced");
  } else {
    document.documentElement.removeAttribute("data-perf-mode");
  }
}
```

```css
/* tokens.css（已定义降级变量） */
[data-perf-mode="reduced"] {
  --blur-panel: blur(0px);
  --blur-dialog: blur(0px);
  --shadow-sm: none;
  --shadow-md: none;
  /* 动效组件通过 useReducedMotion() 检测此状态 */
}
```

---

## 关键 UX 设计细节

### 反向精准锚定

从 Routing 页点击置灰的「高级模式」→ 跳转到 AdvancedPanel → 「启用拓扑图功能」行触发**三次流光呼吸高亮**，形成防迷路闭环。

实现：通过 URL query 参数或 Pinia store 传递「高亮目标」信息。

---

## 提交节点

```
feat(settings): implement all nine settings panels (模块H)
```
