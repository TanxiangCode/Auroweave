# 模块 B — 基础骨架与代理闭环

> 状态：🔄 进行中 | 优先级：🔴 最高 | 里程碑：M1

---

## 目标

打通「导入订阅 → 生成 config → 拉起 sing-box → 真实翻墙」的完整 end-to-end 通路，建立所有后续模块的工程基座。

**验收标准**：导入真实机场订阅链接，成功连接并访问外网。

---

## 已完成（骨架阶段）

- [x] Tauri 2.0 + Vue 3 + TypeScript 工程初始化
- [x] `.gitignore` / `.gitattributes` / `package.json` 配置
- [x] Design Tokens 双主题 CSS 变量系统
- [x] 全局 TypeScript 类型定义（`src/types/index.ts`）
- [x] 五个 Pinia Store 骨架
- [x] WebSocket 客户端封装（`clash-ws.ts`）
- [x] 四个 IPC 封装层（`src/api/ipc/`）
- [x] Vue Router + 五个视图骨架
- [x] Rust 端 `AppError` + `ApiResponse<T>` 统一结构
- [x] Rust IPC 命令骨架（含 TODO 注释）
- [x] `SidecarManager` 生命周期管理器骨架
- [x] Tauri 无边框透明窗口配置

---

## 待实现任务

### B-1 Sing-box Sidecar 完整实现

**文件**：`src-tauri/src/core/sidecar.rs`

- [ ] 使用 `tokio::process::Command` 真实拉起 sing-box 子进程
- [ ] 从 Tauri `resource_dir()` 解析 `sidecar-bin/` 中的平台对应二进制路径
- [ ] 进程崩溃检测（`child.wait()` 在后台任务中监听）
- [ ] 崩溃自动重启（最多重试 3 次，超出后通知前端）
- [ ] 停止接口（`SIGTERM`，Windows 用 `TerminateProcess`）
- [ ] 系统挂起/唤醒监听（通过 Tauri 事件或 OS 原生 API），唤醒后自动重连

**关键代码路径**：
```rust
// 拉起子进程
let mut child = tokio::process::Command::new(&binary_path)
    .arg("run")
    .arg("--config")
    .arg(&config_path)
    .spawn()?;
```

---

### B-2 sing-box 下载脚本

**文件**：`scripts/download-sidecar.ps1`（Windows）/ `scripts/download-sidecar.sh`（macOS）

- [ ] 从 sing-box GitHub Releases 下载 v1.13.14 对应平台二进制
- [ ] 校验 SHA256 哈希
- [ ] 解压并放置到 `sidecar-bin/windows-x64/` 或 `sidecar-bin/macos-universal/`

---

### B-3 订阅解析器完整实现

**文件**：`src-tauri/src/commands/subscription.rs` + `src-tauri/src/core/parser/`

支持格式：
- [ ] **Sing-box 格式**（原生 JSON，直接合并）
- [ ] **Clash / Mihomo YAML**（解析 `proxies` 字段，转换为 sing-box 出站格式）
- [ ] **V2ray / Base64**（解析 vmess://、vless://、trojan:// 协议链接）
- [ ] 节点名称解析地区标记（香港、HK、🇭🇰 等多种写法）

**格式检测逻辑**：
```rust
// 优先尝试 JSON → sing-box，再尝试 YAML → Clash，最后 Base64 → V2ray
fn detect_format(content: &str) -> SubscriptionFormat { ... }
```

---

### B-4 config.json 生成器

**文件**：`src-tauri/src/core/config_builder.rs`

- [ ] 将解析出的节点列表生成 sing-box `outbounds` 数组
- [ ] 自动生成 `urltest` 自动测速出站（按地区分组）
- [ ] 写入默认路由规则（GeoIP / GeoSite）
- [ ] 写入 ClashAPI 配置（`127.0.0.1:9090`）
- [ ] 写入 DNS 配置（默认 `1.1.1.1`）
- [ ] 支持热重载（修改配置后调用 sing-box ClashAPI `PATCH /configs`）

**隐私保证**：全程无网络请求，纯本地文件操作。

---

### B-5 ClashAPI HTTP 客户端

**文件**：`src-tauri/src/core/clash_api.rs`

对接 sing-box ClashAPI（HTTP，非 WebSocket）：
- [ ] `GET /proxies` → 获取分组列表
- [ ] `GET /proxies/{name}` → 获取分组内节点
- [ ] `PUT /proxies/{name}` → 切换节点
- [ ] `GET /configs` → 获取当前配置（含 mode）
- [ ] `PATCH /configs` → 修改配置（切换模式、热重载）

---

### B-6 前端订阅导入 UI

**文件**：`src/views/SettingsView.vue` → `panels/SubscriptionPanel.vue`（骨架）

- [ ] 输入框（URL）+ 名称输入 + 导入按钮
- [ ] 导入中的 loading 状态
- [ ] 导入成功/失败提示

---

### B-7 代理节点列表前端接入

**文件**：`src/views/ProxiesView.vue`（从占位升级为真实内容）

- [ ] 调用 `proxyStore.fetchGroups()` 获取分组
- [ ] 渲染分组卡片列表（简单列表，非最终设计）
- [ ] 节点列表展示（简单行列表，非最终卡片设计）

---

### B-8 ESLint + Prettier 配置

**文件**：`.eslintrc.cjs` / `.prettierrc`

- [ ] ESLint Vue 官方推荐规则集（`plugin:vue/vue3-recommended`）
- [ ] TypeScript 规则（`@typescript-eslint/recommended`）
- [ ] Prettier 格式化规则（单引号、2 空格缩进、尾逗号 ES5）
- [ ] 与 `npm run lint` 脚本对接

---

### B-9 CI 配置

**文件**：`.github/workflows/ci.yml`

- [ ] 触发条件：`push` 到 `develop`、`main`；`pull_request`
- [ ] Job 1：前端 lint（`npm run lint`）
- [ ] Job 2：前端单元测试（`npm run test:unit`）
- [ ] Job 3：Rust 格式检查（`rustfmt --check`）
- [ ] Job 4：Rust clippy（阻断项）
- [ ] Job 5：Rust 单元测试（`cargo test`）

---

## 依赖关系

- **前置**：模块 A（sing-box 兼容性确认）
- **本模块完成后解锁**：模块 C、D、E、F、G、H（全部依赖此代理通路）

---

## 关键技术风险

| 风险 | 应对措施 |
|---|---|
| sing-box 子进程权限不足（TUN 模式需管理员/root） | 先实现系统代理模式，TUN 模式在模块 H 的 TunPanel 单独处理 |
| Clash/V2ray 订阅格式解析不完整 | 先支持最常见的 Mihomo YAML + Base64，其余格式迭代补充 |
| ClashAPI 与 sing-box 1.13.14 响应格式差异 | 参考模块 A 的兼容性报告，做字段防御性读取 |

---

## 提交节点

完成所有任务后，触发 **M1 代理闭环** 提交：
```
feat(core): complete proxy pipeline end-to-end (M1)
```
