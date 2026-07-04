# Auroweave 架构总览

> 作者: TanXiang | 最后更新: 2026-07-04

---

## 目录

- [整体架构](#整体架构)
- [前端架构](#前端架构)
- [后端架构](#后端架构)
- [通信机制](#通信机制)
- [数据流图](#数据流图)
- [关键设计决策](#关键设计决策)

---

## 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                   Vue 3 + TS 前端界面                     │
│  Dashboard │ Proxies │ Routing │ Audit │ Settings        │
│  ┌─────────────────────────────────────────────────────┐│
│  │  Pinia Stores  │  Vue Router  │  Composables        ││
│  └────────────────────┬────────────────────────────────┘│
└───────────────────────┼─────────────────────────────────┘
                        │
         ┌──────────────┴──────────────┐
         │  控制轨（低频安全操作）        │  数据轨（高频实时流）
         │  Tauri IPC Commands          │  本地 WebSocket
         │  invoke("xxx", args)         │  ws://127.0.0.1:9090
         └──────────────┬──────────────┘
                        │
┌───────────────────────┴─────────────────────────────────┐
│              Tauri 2.0 后台 (Rust)                       │
│  commands/  │  core/  │  speedtest/  │  network/        │
│  - 参数校验与调用编排                                      │
│  - 系统提权与 TUN 网络接管                                 │
│  - 进程名/应用路径审计（sysinfo）                          │
│  - 节点测速调度（串行）                                    │
│  - 连接计数统计                                           │
└──────────────────────────────────────────────────────────┘
                        │
                        │  子进程管理（sidecar）
                        ▼
┌──────────────────────────────────────────────────────────┐
│              Sing-box 1.25.4 核心进程 (Go)               │
│  - 流量转发与 L7 嗅探                                     │
│  - 二进制规则匹配 (.srs)                                  │
│  - 内置独立 DNS 路由引擎                                  │
│  - ClashAPI 兼容 HTTP/WebSocket 接口（端口 9090）         │
└──────────────────────────────────────────────────────────┘
```

---

## 前端架构

### 目录职责

```
src/
├── api/                  # 外部通信层（组件绝对不直接穿透此层）
│   ├── ipc/              # Tauri invoke() 封装，按领域拆分
│   └── clash-ws.ts       # WebSocket 订阅封装（流量/连接/日志三路）
├── composables/          # 纯逻辑复用（无 UI 依赖）
│   ├── useReducedMotion  # 动效降级统一判断
│   ├── useFluidWave      # 能量核流体动画驱动
│   ├── useHotkey         # 全局热键绑定
│   └── useMorphTransition # 设置面板变形转场
├── components/           # 可复用 UI 组件
│   ├── chrome/           # 无边框窗口控制层
│   ├── command-palette/  # Spotlight 命令框
│   ├── proxy/            # 节点卡片、测速按钮
│   ├── routing/          # AppMatrixList / TopologyCanvas
│   ├── audit/            # 语义化规则卡片 / 原始日志流
│   └── charts/           # 网速曲线图表
├── stores/               # Pinia 领域状态（与 UI 解耦）
├── views/                # 五个页面级视图（仅负责组合组件）
├── styles/
│   └── tokens.css        # Design Tokens（全局唯一颜色/圆角/动效来源）
├── types/                # 全局 TS 类型（数据契约）
├── constants.ts          # 应用级常量（禁止魔法值散落）
└── router/               # 路由配置
```

### 状态管理原则

| 状态类型 | 存放位置 |
|---|---|
| 跨组件共享的数据 | Pinia Store |
| 单组件内部 UI 状态 | `ref` / `reactive` |
| 从父到子的配置 | `props` |
| 跨层级的事件 | Pinia Action 或 `provide/inject` |

### 组件通信规范

```
组件 → Store Action → IPC 封装层 → Tauri invoke() → Rust
                    ↑
              禁止跨越此边界直接调用
```

---

## 后端架构

### 模块职责

```
src-tauri/src/
├── commands/             # IPC 入口层（仅参数校验 + 调用编排）
│   ├── proxy.rs          # 代理节点与分组操作
│   ├── subscription.rs   # 订阅导入/刷新/删除
│   ├── settings.rs       # 应用设置读写
│   └── speedtest.rs      # 测速任务触发
├── core/                 # 核心业务逻辑
│   ├── sidecar.rs        # sing-box 子进程生命周期管理
│   ├── config_builder.rs # config.json 生成与规则合并
│   └── connection_counter.rs # 真实连接计数（Audit 页数据源）
├── speedtest/            # 测速引擎
│   ├── latency.rs        # 延迟测速（urltest 封装）
│   ├── throughput.rs     # 吞吐量测速
│   └── scheduler.rs      # 串行调度器
├── network/              # 网络状态监听
│   └── ssid_watcher.rs   # Wi-Fi SSID 变化监听
├── system/               # 平台原生 API 封装
│   ├── windows.rs        # Windows 进程审计、UWP 包名识别
│   └── macos.rs          # macOS 辅助功能权限
├── automation/           # 场景自动化规则引擎
├── update/               # 内核版本管理与签名校验
├── tray.rs               # 系统托盘（Windows）/ 菜单栏（macOS）
├── error.rs              # 统一 AppError + ApiResponse<T>
├── lib.rs                # Tauri Builder 入口（注册所有插件和命令）
└── main.rs               # Binary 入口
```

### 错误处理规范

```rust
// ✅ 正确
pub async fn fetch_data() -> Result<Data, AppError> {
    let data = some_io().await.map_err(AppError::from)?;
    Ok(data)
}

// ❌ 禁止
pub async fn fetch_data() -> Data {
    some_io().await.unwrap()  // 生产代码中禁止
}
```

---

## 通信机制

### 双轨分离

| 轨道 | 协议 | 用途 | 特点 |
|---|---|---|---|
| **控制轨** | Tauri IPC (`invoke`) | 切换节点、导入订阅、修改设置 | 低频、有返回值、有错误处理 |
| **数据轨** | WebSocket（直连 sing-box） | 实时流量、连接列表、日志 | 高频、单向推送、需断线重连 |

### WebSocket 数据流

```
sing-box ClashAPI
  ├── ws://127.0.0.1:9090/traffic     → TrafficSnapshot（每秒推送）
  ├── ws://127.0.0.1:9090/connections → Connection[]（变化时推送）
  └── ws://127.0.0.1:9090/logs        → 原始日志行（实时流）
```

### IPC 命令命名规范

```
格式：模块_动作（snake_case，全英文）

示例：
  proxy_get_groups        代理分组列表查询
  proxy_select_node       切换节点
  speedtest_run_single    单节点测速
  subscription_import     导入订阅
  settings_save           保存设置
```

---

## 数据流图

### 订阅导入流程

```
用户输入 URL
    ↓
Vue 组件 → subscriptionStore.importSub()
    ↓
src/api/ipc/subscription.ts → importSubscription()
    ↓
Tauri invoke("subscription_import")
    ↓
Rust commands/subscription.rs → subscription_import()
    ↓
HTTP 拉取订阅内容（reqwest，30s 超时）
    ↓
本地解析节点（Clash/V2ray/Sing-box/Mihomo 格式）
    ↓
core/config_builder.rs → 生成 config.json（本地，无上传）
    ↓
core/sidecar.rs → 热重载 sing-box
    ↓
ApiResponse<Subscription> 返回前端
    ↓
subscriptionStore 更新本地状态
```

### 实时流量渲染流程

```
sing-box WebSocket /traffic
    ↓ 推送 TrafficSnapshot
clash-ws.ts subscribeTraffic()
    ↓ EMA 平滑
connection.store.ts → smoothDownloadSpeed
    ↓ 响应式绑定
useFluidWave.ts → rotationDeg（rAF 驱动）
    ↓
DashboardView 能量核 CSS 动画
```

---

## 关键设计决策

### 1. 双主题 Token 独立设计

浅色主题不是深色主题的简单「减淡」，而是单独设计的完整 Token 集合。磨砂玻璃效果在浅色背景下视觉上有独立考量（减少模糊强度、调整边框对比度）。

### 2. 性能模式双入口

装饰性动效降级同时响应两个信号：
- 系统级 `prefers-reduced-motion`（无障碍）
- 软件内「性能模式」开关（低配设备主动选择）

两者通过 `useReducedMotion()` Composable 统一判断，任一为真即降级。

### 3. sing-box 版本锁定

锁定 **1.25.4**，原因：负载均衡出站 API 格式在不同版本间存在不兼容变更，锁定版本后 CI 回归测试有确定性基准。版本号写入 `constants.ts` 和 `sidecar.rs`，构建脚本从同一常量读取。

### 4. macOS 签名降级策略

v1.0 暂无 Apple Developer 证书，采用 adhoc 自签 + 用户引导（`xattr -cr`）方案。证书申请后通过 CI notarization 步骤升级，不影响功能开发进度。

### 5. UI 与逻辑分离（移动端预留）

所有核心 Composable 和 Rust IPC 命令层不依赖桌面端特有 API（无边框窗口、系统托盘等均封装在独立模块），为未来移动端（Tauri Mobile）接入预留接口。

### 6. 连接统计真实性保障

Audit 页展示的「今日加速次数」等数字来自 Rust 端 `connection_counter.rs` 的真实统计，绝不使用前端摆设的假数字——「绝对透明」是 Auroweave 的核心卖点之一，假数据会严重损害产品可信度。
