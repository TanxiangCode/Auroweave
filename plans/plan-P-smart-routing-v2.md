# 模块 P — 智能分流 v2（DNS evaluate/respond 响应级分流）

> 状态：✅ 已完成 | 优先级：🟠 中高 | 前置：plan-M M3-2
>
> 拟定日期：2026-09-08 | 完成日期：2026-09-08 | 文档依据：`docs/sing-box_docs/dns/rule_action.md`（v1.14.0 精确 tag）
>
> **本文档为设计文档 + 开发说明书合一**：Part A 供评审决策，Part B 供直接动工。

---

## Part A — 设计文档

### A.1 背景与动机

当前分流判定是**域名名单制**（geosite-cn）：域名在名单内 → 国内 DNS 解析 + 直连；不在 → 远端 DoH + 走代理。固有缺陷：**名单永远滞后于现实**——新注册的国内域名不在 geosite 里会被误判为境外流量（走代理，慢且费流量）；部分 CDN 域名误入名单则反向误判。

响应级分流的范式转换：**不看域名在不在名单，而是把域名发给国内 DNS 解析，看答案 IP 的归属**——答案是国内 IP 就直连（解析结果也直接采用），否则按境外处理。判断依据从"域名归属"（静态、滞后）变为"解析结果归属"（动态、实时）。

### A.2 机制（1.14.0 新能力，vendored 文档 + 真实二进制双重核实）

三个新动作构成链条：

| 组件 | 位置 | 语义 |
|---|---|---|
| `evaluate` | 规则动作 | 向指定 server 发查询并**保存响应**，**不终止**后续规则匹配（route 是终态，evaluate 是"边问边继续"）|
| `match_response` | 规则匹配器 | 开启后该规则对**已保存的响应**匹配而非原始查询；配 `ip_cidr`/`rule_set` 判断答案 IP 归属 |
| `respond` | 终态动作 | 把 evaluate 保存的响应直接作为 DNS 答案返回，不再发新查询 |

目标规则链（完整形态）：

```json
"dns": {
  "servers": [ ...既有 local/remote 服务器不变... ],
  "rules": [
    { "clash_mode": "Direct", "action": "route", "server": "local" },
    { "action": "evaluate", "server": "local" },
    { "match_response": true, "rule_set": ["geoip-cn"], "action": "respond" },
    { "action": "route", "server": "remote" }
  ],
  "final": "remote"
}
```

语义读法：先问本地 DNS → 答案命中 geoip-cn（国内 IP）→ 直接采用这个答案（后续路由按 IP 判定直连）；否则走远端 DoH（域名经代理解析）。

### A.3 实测结论（2026-09-08，真实 1.14.0 二进制，全部已验证）

| # | 验证项 | 结果 |
|---|---|---|
| 1 | `evaluate → match_response + ip_cidr 字面量 → respond` 链 | ✅ check 零告警 |
| 2 | `match_response + rule_set 引用 geoip-cn`（复用项目已有 .srs 缓存） | ✅ check 零告警——**设计采用此形态**，与主配置 route 规则共用同一 geoip-cn rule_set |
| 3 | `match_response` 无先行 evaluate | ❌ 拒载（`rule-set not found` 类配置错误在 dns router 初始化期拦截）|
| 4 | `evaluate` 放入 logical 子规则 | ❌ 拒载，报错原文 `dns.rules[0].rules[0]: DNS rule action is not supported in nested rules` |
| 5 | `respond` 无 evaluate 时运行时行为 | 文档明确：请求报错而非回退——配置生成器必须保证链条顺序 |

**关键设计解放**：rule_set 引用合法意味着 CN 判定**复用项目已缓存的 geoip-cn.srs**（订阅激活时下载的那份，`config/geoip-cn.srs`），无需维护两套 IP 段数据。

### A.4 关键设计决策

1. **UI 只暴露一个预设开关**（plan-M 原定调子）：`dns_smart_routing_v2: bool`——「智能分流 v2（按解析结果判定直连）」。**不暴露** evaluate/tag/match_response 任何原生概念。理由：这套语义的 UI 表达成本远超收益，且高级调参可手改 config.json（M3-3 编辑器正好承接这个出口）
2. **默认关闭**：v2 改变全部分流行为语义，灰度从 opt-in 开始；副标题明确写"开启后 CN 判定从域名名单升级为解析结果归属"
3. **开关粒度是 DNS 规则层**，不动 route.rules：直连/代理的路由判定本就按解析出的 IP 走（geoip-cn 路由规则），v2 只改变"域名怎么解析、答案采不采用"——两层正交，v2 开启时路由面零改动
4. **与现有前置规则的顺序**：Direct 模式门控规则（`clash_mode: Direct → local`）保持在链条最前（TUN 场景 Direct 模式下 DoH detour 不经 route.rules 的历史审计结论，不可破坏）
5. **evaluate 的查询成本**：所有域名都会先打一次本地 DNS（evaluate 无条件执行）。开启 v2 后每个境外域名多一次本地 DNS 查询（~10-50ms 级，本地 ISP DNS）；Acceptable——这正是范式转换的固有代价，副标题如实说明

### A.5 风险与对策

| 风险 | 对策 |
|---|---|
| 链条顺序错误 → 内核拒载 | 生成器固定顺序拼装 + 每次生成后 `sing-box check` 兜底（既有工作流）|
| geoip-cn rule_set 未就绪（无订阅态/首启未下载） | 生成器前置检查：无订阅态不注入 v2 链（保持旧规则）；`build_full_route_rules(has_geoip)` 已有 has_geoip 信号可复用 |
| respond 运行时无响应报错（理论上 evaluate 必先行，但防御） | 链条由生成器整块注入，不存在"有 respond 无 evaluate"的组合；rebuild 路径同样整块替换 |
| 本地 DNS 被污染场景答案不可信 | v1 语义（纯名单制）开关随时切回；不叠加 fakeip 等高级面 |
| evaluate 与 optimistic cache 交互 | 实测期重点观察项（evaluate 自带 disable_optimistic_cache 覆盖项，首版不启用任何覆盖）|

---

## Part B — 开发说明书

### B.1 文件级改动清单

| 文件 | 改动 |
|---|---|
| `src-tauri/src/commands/settings.rs` | `AppSettings` 增 `dns_smart_routing_v2: bool`（serde default false）；`rebuild_config_from_settings` 的 dns 段增注入分支 |
| `src-tauri/src/core/config_builder.rs` | `with_dns` 增 `smart_v2: bool` 参数；`build` 的 dns.rules 拼装增 v2 链 |
| `src/views/panels/DnsPanel.vue` | 开关一行（仿既有 optimistic checkbox 形态） |
| `src/types/index.ts` | settings 类型补字段 |
| `src-tauri/src/commands/settings.rs` | （已有）`group_update_config` 等不需要动——v2 重建配置的触发点 = settings_save 后的既有 rebuild 链 |

### B.2 后端实现要点

**ConfigBuilder 侧**（`with_dns` 改造）：

```rust
pub fn with_dns(mut self, remote_doh: String, timeout_secs: u64, optimistic: bool, smart_v2: bool) -> Self {
    // ...既有字段存储...
    self.smart_v2 = smart_v2;  // ConfigBuilder 加字段
    self
}
```

`build()` 的 dns.rules 拼装（**顺序不可变**）：

```rust
let mut dns_rules = vec![];
dns_rules.push(json!({"clash_mode": "Direct", "action": "route", "server": "local"})); // 既有前置
if self.smart_v2 && self.has_geoip {   // has_geoip: 主配置 route.rule_set 已含 geoip-cn
    dns_rules.push(json!({"action": "evaluate", "server": "local"}));
    dns_rules.push(json!({"match_response": true, "rule_set": ["geoip-cn"], "action": "respond"}));
    dns_rules.push(json!({"action": "route", "server": "remote"}));
} else {
    // 既有节点域名/geosite-cn 两条规则原样保留
}
```

调用点同步：`build_and_apply_config`（subscription.rs）构造 ConfigBuilder 时传入 settings 值——**搜索 `with_dns(` 全部调用点逐一补参**（上轮 M3-1 双路径漂移的教训）。

**rebuild 侧**（`rebuild_config_from_settings` 的 dns 段）：与 ConfigBuilder 同语义整块替换 `dns.rules`（v2 开启且 route.rule_set 含 geoip-cn 时注入三行链，否则恢复既有规则）。**两个生成路径的注入逻辑写成一个共享函数** `build_dns_rules(smart_v2: bool, has_geoip: bool) -> Vec<Value>`，两侧调用——这是防漂移的结构性手段，比"改两处+注释提醒"可靠。

### B.3 前端实现要点

DnsPanel 增一行（复用既有 setting-item + checkbox 形态）：

```vue
<div class="setting-item">
  <div class="item-label">
    <span>智能分流 v2 (Smart Routing)</span>
    <span class="sub-label">按解析结果判定直连：先问本地 DNS，答案为国内 IP 则直连——比域名名单（geosite）更实时，对新域名/CDN 误判免疫</span>
  </div>
  <input type="checkbox" v-model="settingsStore.settings.dns_smart_routing_v2" class="switch" @change="save" />
</div>
```

保存走既有 `save` → settings_save → rebuild 链（重启内核生效，弹窗文案提示"将在内核重启后生效"——rebuild 链已有该语义）。

### B.4 验证计划（按项目验证工作流）

1. **cargo 单测**：`build_dns_rules` 共享函数——v2 开/关 × has_geoip 有/无 四象限断言
2. **sing-box check 实测**：v2 开启的完整 builder 配置过真实二进制零告警（规则链形态已在 A.3 实测预验证）
3. **行为验证**（核心验收）：v2 开启后拉起内核——
   - `dig baidu.com`（CN 域名）@127.0.0.1:8890 应返回本地 ISP DNS 的答案（直连解析），nslookup 日志确认无 remote 服务器查询
   - `dig google.com` 应返回经代理解析的结果
   - 既有 geosite-cn 域名分流、Direct 模式前置规则回归不破坏
4. **切换回归**：开关来回切换各一次，确认 rebuild 后规则链正确替换、无残留混合状态
5. **vue-tsc / vite build / vitest** 全套

### B.5 验收标准

- [x] 开启后 CN 域名直连判定不再依赖 geosite 名单（dig 实测：baidu/qq/bilibili/taobao 均走 evaluate→respond 链返回国内 CDN IP，测试配置 dns.rules 无任何名单规则）
- [x] 抽样验证：bilibili/taobao 等域名 v2 下直连判定准确（真实生产环境下新旧域名命中率对比留观察期；测试环境 google.com 答案 174.132.167.252 非 CN，正确 fallthrough 远端）
- [x] 关闭后配置与改动前语义等价（v1 分支恢复 geosite 名单规则，单测锁定）
- [x] has_geoip 为 false（无订阅态）时不注入 v2 链、不拒载（单测 + check 实测）
- [x] 真实 221 节点 config.json 迁移 v2 链后 sing-box check 零告警
- [x] cargo 59/59、vue-tsc、vite build、vitest 6/6 全绿

### B.6 明确不做

- ❌ evaluate 的 tag/speculative/race 组合（多服务器竞技语义，自用无场景）
- ❌ UI 暴露原生规则字段
- ❌ fakeip 与 v2 的组合验证（正交功能，各管各的）
