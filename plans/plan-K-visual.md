# 模块 K — 视觉设计系统与转场动效

> 状态：🔄 部分完成（Design Tokens 初稿已提交） | 优先级：🟠 高 | 可与其他模块并行

---

## 目标

建立完整的视觉设计系统（Design Tokens 双主题、能量核动画、无边框窗口），并实现全局微交互基础规范。

**验收标准**：
- 性能模式开启后，所有装饰性动效降级为静态或简单过渡
- 浅色/深色主题下磨砂玻璃效果均有良好可读性和对比度
- macOS 原生红绿灯区域可正常拖拽，无误触

---

## 已完成（骨架阶段）

- [x] `src/styles/tokens.css` — 深色+浅色双主题 Design Tokens 初稿
- [x] `src/composables/useReducedMotion.ts` — 动效降级统一判断
- [x] `src/composables/useFluidWave.ts` — 能量核流体动画 Composable（含 EMA / 可见性暂停 / 降级）
- [x] `src/components/chrome/ControlCapsule.vue` — 无边框窗口控制胶囊（最小化/最大化/关闭按钮 + 设置入口 + 防误触隔离）
- [x] `src/components/chrome/TrafficLights.vue` — macOS 原生红绿灯安全支持层

---

## 待实现任务

### K-1 Design Tokens 完善

**文件**：`src/styles/tokens.css`（升级完整版）

- [ ] WCAG AA 对比度检查：`--accent-red #EF4444` 在 40% 透明磨砂玻璃面板上的文字对比度（需达到 4.5:1）
- [ ] 若不达标，调整 `--accent-red` 值或在磨砂玻璃背景上增加文字阴影
- [ ] 浅色主题下验证：磨砂玻璃效果是否成立（在白色背景上的 `backdrop-filter: blur` 效果）
- [ ] 接入 Tailwind config：将 Token 变量映射到 Tailwind 工具类

```javascript
// tailwind.config.js
module.exports = {
  theme: {
    extend: {
      colors: {
        "layer-0": "var(--layer-0)",
        "accent-blue": "var(--accent-blue)",
        // ...
      },
      borderRadius: {
        "lg": "var(--radius-lg)",
        // ...
      }
    }
  }
}
```

---

### K-2 无边框窗口跨平台实现

**文件**：`src/components/chrome/ControlCapsule.vue` + `src/components/chrome/TrafficLights.vue`

#### Windows 实现

- [ ] 拖拽区域：`data-tauri-drag-region` 属性标注标题栏区域
- [ ] 双击最大化（监听 `dblclick` 事件 + Tauri `getCurrentWindow().maximize()`）
- [ ] 自定义最小化/最大化/关闭按钮
- [ ] **Snap Layout 兼容性验证**（鼠标悬停最大化按钮时弹出 Win11 分屏菜单）
  - 参考：Tauri 是否有 `WS_MAXIMIZE_BOX` 相关 flag 保留 Snap Layout
  - 结论写入权限预研报告姊妹文档

#### macOS 实现

- [ ] **保留原生红绿灯样式**（`hiddenTitle: true` + 不覆盖原生控件）
- [ ] 在红绿灯右侧预留 ≥8px 安全间距，放置设置齿轮图标
- [ ] 验证：红绿灯区域可正常拖拽窗口、无误触设置图标

---

### K-3 全局控制胶囊

**文件**：`src/components/chrome/ControlCapsule.vue`

```
右上角控制胶囊布局（非展开态）：
[ ⚙️ ]  ←  ←  ←  [─  ⬜  ✕]
         ≥60px 防误触间距
```

**展开态（卡片全屏后）**：
```
左侧追加纵向三图标切换器：
[ 🌐 ]  ← 代理节点
[ 🛠️ ]  ← 分流配置
[ 🔍 ]  ← 安全审计
```

- [ ] 胶囊悬浮在所有内容之上（`z-index: 1000`）
- [ ] 设置图标与关闭按钮物理距离 > 60px（防误触规范）
- [ ] 展开态三图标切换器（卡片膨胀为全屏后显示，无需回到 Dashboard）

---

### K-4 能量核完整实现

**文件**：`src/views/DashboardView.vue`（升级）

当前骨架为 CSS 动画，升级为 `useFluidWave` Composable 驱动：

- [ ] 将 `smoothDownloadSpeed` 从 `connectionStore` 传入 `useFluidWave`
- [ ] `rotationDeg` 绑定到 CSS 自定义属性 `--ring-rotation`
- [ ] conic-gradient 通过 `--ring-rotation` 实现旋转（`background: conic-gradient(from var(--ring-rotation), ...)`）
- [ ] 三种状态视觉：未连接（呼吸灯暗灰）/ 连接中（粒子加速旋转）/ 已连接（青色流体）
- [ ] 底部策略组快速切换胶囊（最近使用的 3-4 个，来自 `proxyStore`）

---

### K-5 设置面板转场预研 Demo

**文件**：`src/composables/useMorphTransition.ts`

- [ ] 实现 View Transitions API 版本（优先）
- [ ] 实现 FLIP 手写动画版本（备用）
- [ ] 在 Tauri WebView2（Windows）+ WKWebView（macOS）各测试帧率
- [ ] 评估结论，选择最终方案并记录

---

### K-6 微交互基础规范

**文件**：`src/styles/micro-interactions.css`（新建，在 `main.ts` 中导入）

三条全局微交互规则，适用于所有可交互元素：

```css
/* Hover 阻尼（150ms 内背景色变化 3%） */
.interactive {
  transition: background-color var(--duration-fast) var(--ease-default);
}

/* 点击阻尼（物理按键感） */
.interactive:active {
  transform: scale(0.98);
  transition: transform var(--duration-instant);
}
```

- [ ] 定义 `.interactive` 基础类
- [ ] 所有按钮、卡片、胶囊默认应用此类
- [ ] 验证：不与 `:disabled` 状态冲突

---

### K-7 无障碍与性能兜底

- [ ] 高对比度选项（`PrivacyPanel.vue` 中，独立 CSS 变量覆盖层）
- [ ] 验证性能模式下 `backdrop-filter: blur(0px)` 实际生效（测试渲染性能）
- [ ] WCAG AA 对比度检查工具：[contrast-ratio.com](https://contrast-ratio.com)，所有关键文字色组合需通过

---

## 视觉一致性检查清单

在各模块开发完成后，统一过一遍：

- [ ] 所有弹窗/对话框使用磨砂玻璃卡片（非原生系统弹窗）
- [ ] 所有颜色来自 Design Token（`grep -r "color:" src/ | grep -v "var(--"` 不得有结果）
- [ ] 测速中的进度动画与能量核视觉语言统一（流体色彩，非普通 loading spinner）
- [ ] 深色 + 浅色主题各截图一次，确认无色彩崩坏

---

## 提交节点

```
feat(visual): complete design system and window chrome (模块K)
```
