# 模块 G — Audit 页面与环境自动化

> 状态：⏳ 待开始 | 优先级：🟡 中 | 前置：模块 B、C

---

## 目标

提供语义化的安全看板（将冰冷规则翻译为自然语言）、真实的连接统计数字，以及基于 Wi-Fi SSID 的场景自动化规则引擎。

**验收标准**：
- 常见规则（GeoIP / 直连 / 代理）正确翻译为语义化卡片
- 自定义规则有可读的兜底展示，不出现空白或报错
- 统计数字来自 Rust 端真实计数器，非摆设

---

## 任务清单

### G-1 真实连接计数器

**文件**：`src-tauri/src/core/connection_counter.rs`

> ⚠️ 此模块是 Audit 页「透明度」承诺的基础，必须优先实现，不能用假数字

**计数口径定义**（必须文档化）：
- **代理连接**：出站类型非 `direct` 且非 `block` 的连接，计为一次「加速」
- **直连**：出站类型为 `direct` 的连接
- **拦截**：出站类型为 `block` 的连接
- 计数按**自然日**重置（UTC+8 午夜）

**实现**：
- [ ] 监听 sing-box WebSocket `/connections`，每个新连接根据 `outbound` 字段分类计数
- [ ] 统计数据持久化（SQLite 或简单 JSON，按日期存储）
- [ ] 提供 IPC 命令：`audit_get_stats()` → `ConnectionStats`
- [ ] 单元测试：验证计数逻辑正确性，覆盖边界情况（同日重置、跨日等）

---

### G-2 DNS 审计记录流

**文件**：`src-tauri/src/core/dns_tracker.rs`

- [ ] 解析 sing-box 日志中的 DNS 查询记录（从 `/logs` WebSocket 解析）
- [ ] 或通过 ClashAPI `/connections` 推断规则命中情况
- [ ] 构建 `DnsAuditRecord` 结构（domain / rule / outbound / resolved_ips）
- [ ] 保存最近 500 条记录（内存环形缓冲区）
- [ ] IPC 命令：`audit_get_records()` → `Vec<DnsAuditRecord>`

---

### G-3 语义化翻译层

**文件**：`src/components/audit/SemanticRuleCard.vue` + `src/utils/semantic-translator.ts`

**翻译映射表**（初始版本）：

| 规则类型 | 出站 | 语义文案 | 图标 |
|---|---|---|---|
| `geoip:cn` | direct | 绕过大陆（直接连通） | 🎯 |
| `geoip:*` (非cn) | proxy | 节点加速中（命中海外 IP 规则） | 🚀 |
| `domain_suffix:.google.com` | proxy | 谷歌服务 · 节点加速中 | 🚀 |
| `domain_suffix:.cn` | direct | 国内域名 · 直接连通 | 🎯 |
| `process_name:*` | 任意 | 应用规则（App-Matrix 绑定） | 📱 |
| 任意 | block | 已拦截 | 🚫 |

**兜底文案**（无法匹配预设模板时）：
```
自定义规则 #N 命中 → 出站: HK-01
```
**绝对不能**出现空白卡片或 JS 报错。

- [ ] 翻译函数：`translateRule(record: DnsAuditRecord): { text: string; icon: string }`
- [ ] 兜底逻辑：匹配失败时返回通用格式
- [ ] 单元测试：覆盖所有预设模板 + 兜底情况

---

### G-4 AuditView 视图

**文件**：`src/views/AuditView.vue`

**折叠态（卡片式显示在 Dashboard 三张启动卡片中）**：
```
🛡️ 当前网络环境安全
今日已为您加速 1,420 次连接
```

**展开态（全屏舞台）**：
```
┌─────────────────────────────────────────────────────────┐
│ 🛡️ 安全审计              [🧑‍💻 切换到内核原始日志]       │
├─────────────────────────────────────────────────────────┤
│ 今日已代理: 1,420 次  直连: 3,201 次  拦截: 15 次       │
├─────────────────────────────────────────────────────────┤
│ 14:32:10  github.com    🚀 节点加速中（海外极客规则）     │
│ 14:32:08  baidu.com     🎯 绕过大陆（直接连通）          │
│ 14:32:07  ads.com       🚫 已拦截                        │
└─────────────────────────────────────────────────────────┘
```

- [ ] 统计数字与 `connection_counter.rs` 真实数据绑定
- [ ] 规则卡片时间流（最新在上，虚拟滚动）
- [ ] 时间流自动暂停/恢复（鼠标悬停时暂停，离开后继续）

---

### G-5 原始日志极客后门

**文件**：`src/components/audit/RawLogStream.vue`

- [ ] 切换按钮：「🧑‍💻 切换到内核原始日志」（Audit 页右上角胶囊）
- [ ] 订阅 `subscribeLog()`，实时显示 sing-box 原始日志行
- [ ] 虚拟滚动（防止 DOM 节点过多导致卡顿）
- [ ] 日志级别颜色编码（INFO 白 / WARN 黄 / ERROR 红）
- [ ] 底部「自动滚动」开关（用户向上滚动查看历史时自动停止，点击按钮恢复）

---

### G-6 Wi-Fi SSID 监听

**文件**：`src-tauri/src/network/ssid_watcher.rs`

- [ ] Windows：通过 `WlanGetAvailableNetworkList` 或 `wlanapi` 获取当前 SSID
- [ ] macOS：通过 `CWWiFiClient.sharedWiFiClient().interface()?.ssid()` 获取
- [ ] SSID 变化时触发 Tauri Event → 前端 / 自动化引擎响应

---

### G-7 场景自动化规则引擎

**文件**：`src-tauri/src/automation/`

**规则结构**：
```json
{
  "id": "rule-1",
  "name": "家庭 Wi-Fi 直连模式",
  "enabled": true,
  "trigger": { "type": "ssid", "ssid": "HomeNetwork" },
  "action": { "type": "switch_proxy_mode", "target": "direct" }
}
```

- [ ] 规则持久化（`automation-rules.json`）
- [ ] SSID 变化时匹配规则并执行动作
- [ ] 支持的动作：切换代理模式 / 切换节点分组 / 切换到指定节点
- [ ] IPC 命令：CRUD 操作自动化规则

---

### G-8 系统托盘

**文件**：`src-tauri/src/tray.rs`

- [ ] Windows：系统托盘图标（状态图标：连接中/断开）
- [ ] macOS：菜单栏图标（与 Windows 托盘功能相同）
- [ ] 右键菜单：开关代理 / 切换模式 / 显示主窗口 / 退出
- [ ] 双击托盘图标恢复主窗口

---

## 提交节点

```
feat(audit): semantic audit dashboard and automation engine (模块G)
```
