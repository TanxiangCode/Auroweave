# 模块 J — Routing 高级模式（拓扑画布）

> 状态：🔵 低优先级（可选） | 前置：模块 B、F | **不影响 v1.0 主体发布**

---

## 重要说明

> 此模块不影响 v1.0 版本主体功能上线，可在首个正式版本发布后作为后续迭代补齐。
> 若开发资源紧张，可直接砍掉此模块，**Routing 简单模式（模块 F）完全不受影响**。

---

## 目标

为重度极客用户提供拖拽式多跳链路配置界面，通过 Vue Flow 画布连线生成 sing-box `dialer` 嵌套 JSON。

**验收标准**：
- 开关开启后拓扑画布 Tab 可点击，拖拽连线正确生成多重代理链 JSON
- 关闭开关后 Tab 置灰，不影响简单模式使用

---

## 任务清单

### J-1 性能预研

**前置工作**（在正式开发前完成）：

- [ ] 实测 `@vue-flow/core` 在 50+ 节点连线场景下的渲染性能
- [ ] 确认 Vue Flow 能否在 Tauri WebView2（WebView2/Chromium）中正常运行
- [ ] 评估内存占用（是否在画布不可见时释放资源）

---

### J-2 AdvancedPanel 开关联动

**文件**：`src/views/panels/AdvancedPanel.vue`（已预留）

- [ ] `topology_enabled` 开关实时控制 `RoutingView.vue` 高级模式 Tab 的可用性
- [ ] 关闭时 Tab 置灰，悬浮气泡：「在高级设置 → 启用拓扑图功能中开启」
- [ ] 点击置灰 Tab → 直达 AdvancedPanel + 三次流光高亮目标开关

---

### J-3 拓扑画布组件

**文件**：`src/components/routing/TopologyCanvas.vue`（懒加载）

**组件懒加载**：
```vue
<!-- RoutingView.vue 中只在 topology_enabled = true 时才懒加载 -->
<component :is="() => import('./routing/TopologyCanvas.vue')" />
```

**功能**：
- [ ] 安装 `@vue-flow/core`
- [ ] 节点类型：入口节点（SOCKS5/HTTP）/ 代理节点 / 出口节点
- [ ] 拖拽连线生成代理链
- [ ] 右键菜单：编辑节点参数 / 删除节点 / 断开连线 / 设为全局出口
- [ ] 节点过多时自动淡入 Minimap（`@vue-flow/minimap`）
- [ ] 无级缩放（滚轮 + 触控板）
- [ ] 空画布状态：全息网络流向引导插画 + 「添加节点」引导文字

---

### J-4 Vue Flow ↔ Sing-box JSON 互转

**文件**：`src/utils/topology-converter.ts`

**方向 1：画布 → JSON**
```typescript
// 将 Vue Flow edges（连线）转换为 sing-box 多跳 dialer 嵌套格式
function canvasToSingboxConfig(
  nodes: Node[],
  edges: Edge[]
): SingboxOutbound[] { ... }
```

**sing-box 多跳格式**（嵌套 `detour`）：
```json
[
  {
    "type": "vless",
    "tag": "落地节点",
    "server": "1.2.3.4",
    "detour": "前置节点"
  },
  {
    "type": "shadowsocks",
    "tag": "前置节点",
    "server": "5.6.7.8"
  }
]
```

**方向 2：JSON → 画布**（从已有 config 还原画布状态）
- [ ] 解析 `outbounds` 中的 `detour` 字段，构建节点和连线

---

### J-5 App-Matrix 数据纽带

**文件**：`src/components/routing/OutboundSelector.vue`（来自模块 F）

- [ ] 拓扑画布中创建的「链路」自动出现在 App-Matrix 的出站选择下拉菜单中
- [ ] 链路名称前加 🔗 图标，置顶显示（渐变色区分）
- [ ] 链路的延迟显示为「N 跳（估算）」，而非精确延迟数值

---

## 设计风险

| 风险 | 应对 |
|---|---|
| Vue Flow 在 WebView2 性能问题 | J-1 预研阶段验证，不通过则砍掉此模块 |
| 多跳 JSON 转换逻辑复杂 | 先支持单链路（A→B），再迭代支持分叉（A→B+C） |
| 用户误用（把全部流量路由到复杂链路） | 首次开启时显示一次性告知卡片，说明「普通用户无需使用此功能」 |

---

## 提交节点

```
feat(routing): add topology canvas for advanced multi-hop routing (模块J)
```
