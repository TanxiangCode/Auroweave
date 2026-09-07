# 模块 N — 测试内核实例（test-core）模式

> 状态：✅ 已完成（2026-09-07，N-1~N-5 全部交付；实测含 dns.servers 修正） | 优先级：🔴 高（体验级架构改造） | 前置：解锁检测 v1/v2（已交付，selector 轮换版）
>
> 拟定日期：2026-09-07 | 预估工作量：1.5-2 天

---

## 背景与动机

解锁检测 v1 落地时采用 **selector 轮换**（切主 selector → 经 mixed 端口检测 → 还原），与批量吞吐测速同一范式。该范式的问题：批量检测期间用户实时流量跟着出口轮换，长连接服务（登录态/WS/流媒体）可能因 IP 跳变被踢下线，体感为"能上网但很不稳"。

**根因**：项目为单 selector 架构——mixed 端口只有一个，其出口由主 selector 当前选择决定；需要真实响应内容（状态码/重定向/正文 marker）的检测无法使用 ClashAPI 代测端点（机制 A 对 Gemini 封锁页 200 / Claude 302 落 200 全绿误报），被迫占用用户出口。

**本方案的解法**（v2rayN 批量测速已验证的同型架构，sing-box 能力齐备）：

拉起一个**短生命周期的第二 sing-box 进程（test-core）**，配置为 **N 入站 + N 出站 + N 条 inbound 路由规则**——每个被测节点分得一个专属本地端口，端口流量被规则钉死到对应节点出站。检测器并发请求这些端口，**主实例、主 selector、用户流量全程零打扰**。测试结束杀掉 test-core，一切归零。

关键文档依据（`docs/sing-box_docs/`，v1.14.0 精确 tag）：
- `route/rule.md:219` — `"inbound": [tag]` 路由匹配器
- `inbound/mixed.md` — mixed 入站为标准数组项，多实例天然支持
- 代理协议（vmess/vless/trojan/ss/hy2/tuic）域名由远端解析，测试配置可极简 DNS

### 与已否决方案的对比

| 方案 | 结论 | 原因 |
|---|---|---|
| 机制 A（ClashAPI delay 代测） | ❌ 判据上不可行 | 非 2xx 才算失败；Gemini 封锁页 200、Claude 封锁 302 落 200，误报"可用" |
| selector 轮换 | ✅ v1 已交付，降级为 fallback | 可用但打扰用户流量 |
| 专属 inbound 注入**主配置** | ❌ 否决 | 改配置需重启内核；每节点改指向 = 300 次重启，比轮换更差 |
| **test-core 独立实例** | ✅ 本方案 | 一次进程启动覆盖全批节点；零打扰；并发化 |

---

## 目标

批量解锁检测从"串行 + 出口轮换 + 5-10 分钟"升级为"受控并发 + 零打扰 + 约 1-2 分钟"。

**验收标准**：
- 批量检测期间主 selector 的 `now` 全程不变（ClashAPI 连续采样验证）
- 检测期间用户侧网页浏览、视频播放无感知（人工验收）
- test-core 异常退出（手动 kill）不残留进程/端口/文件，主实例不受影响
- 应用退出时 test-core 必被清理（Exit 钩子验证）
- 300 节点批量检测总耗时 ≤ 3 分钟（ip 层异步另计）
- selector 轮换路径保留为 fallback：test-core 拉起失败时自动降级并 toast 告知

---

## 架构设计

### 组件图

```
┌─────────────────────────── Auroweave 主进程 ───────────────────────────┐
│                                                                        │
│  主 sing-box 实例 (SidecarManager)          test-core 实例 (TestCoreManager)  │
│  ├─ mixed :8890 ← 用户流量                     ├─ mixed :PORT_BASE+0  ─┐   │
│  ├─ TUN (可选)                                 ├─ mixed :PORT_BASE+1  ─┤   │
│  ├─ selector proxy ── 用户选择                 ├─ ...                 │ │ N 条
│  └─ ClashAPI :9090                             └─ mixed :PORT_BASE+k ─┘   │
│                                                route: inbound[i] →        │
│  UnlockCheckScheduler (改造)                    outbound node[i]           │
│  ├─ 测试配置生成 → config_test.json                                       │
│  ├─ TestCoreManager.start() / stop()                                     │
│  ├─ 并发探测 PORT_BASE+i (Semaphore 受控)                                 │
│  └─ ip-api 层：随批并发（出口分散天然规避单 IP 限速）                   │
└────────────────────────────────────────────────────────────────────────┘
```

### 测试配置形态（单批次 B 个节点）

```json
{
  "log": { "level": "warn" },
  "inbounds": [
    { "type": "mixed", "tag": "t-in-0", "listen": "127.0.0.1", "listen_port": 40040 },
    { "type": "mixed", "tag": "t-in-1", "listen": "127.0.0.1", "listen_port": 40041 }
  ],
  "outbounds": [
    { "...节点0完整配置...", "tag": "t-node-0" },
    { "...节点1完整配置...", "tag": "t-node-1" }
  ],
  "route": {
    "default_domain_resolver": "local",
    "rules": [
      { "inbound": ["t-in-0"], "outbound": "t-node-0" },
      { "inbound": ["t-in-1"], "outbound": "t-node-1" }
    ],
    "final": "direct",
    "auto_detect_interface": true
  }
}
```

设计决策：
- **无 experimental / clash_api / cache_file**：test-core 无需控制面；与主实例的 9090、cache.db 零冲突
- **无 TUN**：测试配置永不包含 TUN，绕开 SUID/服务/提权整条链路（sidecar `start` 的 TUN 特权分支天然不触发）
- **`log.level: warn`**：测试探测是海量短连接，info 级日志会刷爆 ring buffer
- **outbound tag 加 `t-` 前缀**：与主配置 tag 空间隔离，规避"订阅刷新双写"问题（test-core 读的是生成时刻的快照配置，订阅刷新只影响主实例）
- **批次分片**：单批次上限 32 节点（32 端口），超过分批重启 test-core。300 节点 = 10 批，每批进程重启约 0.3s（就绪探测实测 230ms 基线，见第七轮性能记录），总开销 ~3s 可忽略

### 端口策略

- 基址 `40040`（settings 新增 `test_core_port_base`，默认 40040，可避开冲突）
- 启动前逐端口 `TcpListener::bind` 试占验证，冲突则整体基址 +1000 重试（最多 3 次）
- 进程退出后由 OS 释放，无手动清理需求

### 并发模型

- **服务探测层**（Gemini/Claude/ChatGPT）：`Semaphore` 受控并发，默认 8（settings 新增 `unlock_test_concurrency`，默认 8，范围 2-16）——三服务是轻量页面请求，节点带宽压力可忽略
- **ip-api 层**：**随批次并发**（与服务探测同 Semaphore 约束），另加全局 ip 请求在飞上限 16 作礼貌兜底。关键认知：ip-api 限速按**客户端出口 IP** 计数（45 req/min per IP），test-core 下每个查询走专属端口直达对应节点出口——300 节点 = ~300 个不同客户端各 1 次请求，天然分散，单出口几乎不可能触限（例外见风险表"同出口多节点"行）。**v1 的全局 1.5s 节流在轮换架构下即已过度保守（请求本就来自各节点出口），本方案取消**
- **多源容错（429 触发式）**：不做预先轮转；ip-api 返回 429/超时时单发换 freeipapi 重试一次（60/min、含 isProxy 字段；hosting 字段缺失可接受），再失败则该节点 IP 层留空、不阻塞服务层结果
- 每节点三服务探测可再内层并发（tokio::join!），外层 Semaphore 控节点级并发

---

## 任务清单

### N-1 测试配置生成器

**文件**：`src-tauri/src/core/test_core.rs`（新建）

- [x] `pub fn build_test_config(nodes: &[ParsedOutbound], port_base: u16) -> Result<Value, AppError>`
  - 入站：`t-in-{i}` mixed × N；出站：`raw_json` 克隆 + `t-node-{i}` tag 改写（复用 `config_builder.rs:235` 的 raw_json 处理方式）；路由：`inbound → outbound` 逐条映射
  - 节点过滤：`is_valid_proxy_node` + `is_announcement_or_fake_node`（复用 `parser/mod.rs:109/170`），保持与主配置同一过滤语义
  - 无 dns 段裸奔风险：显式 `"default_domain_resolver": "local"`（1.14 强制项，缺失直接 FATAL——见 singbox 升级记忆实测结论）
- [x] `pub fn plan_batches(nodes: &[ParsedOutbound], batch_size: usize) -> Vec<Vec<usize>>`（节点索引分片，32/批）
- [x] 写盘 `config_test.json`（临时路径，进程启动前原子写、结束后删除；路径 `get_config_dir().join("config_test.json")`）
- [x] 单测：生成配置经 `sing-box check` 实测零告警（纳入验证工作流）；tag 前缀隔离、路由条数 = 节点数断言

### N-2 TestCoreManager 进程管理

**文件**：`src-tauri/src/core/test_core.rs`（同文件，或独立 `core/test_core_manager.rs`）

- [x] `pub struct TestCoreManager`：`spawn(config_path) / stop() / is_running()` + 内部 `Mutex<Option<Child>>`
  - 二进制定位：复用 `SidecarManager::resolve_binary_path()`（`sidecar.rs:583`，pub 已存在）
  - spawn 参数：`sing-box run -c config_test.json`；**不经过** `SidecarManager::start`（其状态机/ClashAPI 就绪探测/watchdog 均为主实例语义，测试实例只需要"起来了吗"）
  - 就绪探测：逐端口裸 TCP connect（复用 `sidecar.rs:45` 探测思路），首个端口连通即就绪；上限 3s 超时
  - 早期退出检测：spawn 后 500ms `try_wait()` 一次（配置错误秒级退出，错误信息从 stderr 尾部截取上报）
  - **进程绑定**：Windows 复用 Job Object（`system/job.rs`）；macOS/Linux 依赖 stop + Exit 钩子双保险
  - stdout/stderr：丢弃（level=warn 无噪音；避免再挂一套 ring buffer）
- [x] stop：kill + wait(3s) + 删配置文件 + 释放状态；幂等
- [x] **Exit 钩子接线**（`lib.rs:242` RunEvent::Exit）：在现有清理序列中追加 `test_core_manager.stop()`（服务模式分支同样要清——test-core 与运行模式无关）
- [x] 与主 SidecarManager 完全解耦：互不知晓、互不干扰；test-core 崩溃不影响主实例状态机

### N-3 UnlockCheckScheduler 改造（核心）

**文件**：`src-tauri/src/commands/unlock_check.rs`（改造现有）

- [x] `run_batch` 重写为 test-core 路径：
  1. `collect_active_outbounds()` 拿全量节点（`subscription.rs:246`，pub 化——当前为私有 fn，需提升可见性）
  2. 与前端传入的 `node_tags` 求交集（tag 匹配；未命中者记 Failed——订阅刚刷新、节点已消失）
  3. `plan_batches` 分片 → 逐批：`build_test_config` → `TestCoreManager::spawn` → 并发探测 → `stop`
  4. 探测任务：`check_via_port(port, services, params)`——新函数，复用 `unlock_check.rs` 的 `fetch_service_response` / `classify_response` / `fetch_egress_info`（改造 `check_current_exit` 接受动态端口参数，原 mixed_port 调用点改传参）
  5. 进度事件沿用 `unlock-check-progress`（current_index 跨批累计），前端零改动
  6. ip 层：每节点探测任务内联并发执行（服务探测 + ip 查询 join! 后一次性产出完整 result），同一 Semaphore 约束；进度事件协议不变（payload 一次携带完整结果）
- [x] **fallback 降级**：`build/spawn/就绪探测` 任一步失败 → 日志 + toast 事件 → 整批降级走 v1 selector 轮换路径（原代码保留为 `run_batch_selector_legacy`）
- [x] **互斥**：与 `SpeedTestScheduler` 之间不需要互斥（新路径不碰 selector）；但 test-core 自身全局单实例锁（两批并发 spawn 会端口打架）
- [x] 取消：现有 cancel channel 保留；批次边界 + permit 边界双重检查取消信号
- [x] 单节点检测 `unlock_check_single` 同步切换 test-core 路径（1 节点 = 1 端口单批），零打扰收益同样覆盖单测场景

### N-4 settings 与命令面

- [x] `AppSettings` 新增：`test_core_port_base: u16`（默认 40040）、`unlock_test_concurrency: u32`（默认 8）；serde default 平滑兼容
- [x] `commands/mod.rs` / `lib.rs`：`collect_active_outbounds` pub 化；无新命令（复用现有四条 unlock 命令，路径选择对前端透明）
- [x] 前端零改动（事件协议不变）；仅确认弹窗文案微调（去掉"出口轮换"警示，改为"将启动临时测试内核"）

### N-5 （可选增强，独立决策点）吞吐测速同架构迁移

批量吞吐测速同属"经端口的业务流量"范式，可迁 test-core 获得零打扰收益。**但**：吞吐测速并发会互相抢带宽，测速数值失真——v2rayN 用户对此抱怨已久。建议：迁移但**默认并发 1-2**（settings `speedtest_test_concurrency` 默认 1 = 串行语义不变，仅获零打扰收益）。此项 0.5 天，可随 N 批次或独立立项。

---

## 风险与对策

| 风险 | 对策 |
|---|---|
| 端口被占（其他应用/本机代理冲突） | 启动前 bind 试占，基址 +1000 重试 3 次；再失败降级 fallback |
| 订阅节点配置含主配置专属字段（如 dns 引用） | raw_json 来自解析器输出，天然纯净；生成后 `sing-box check` 实测兜底 |
| test-core 进程残留（崩溃路径） | Exit 钩子 + stop 幂等 + 下次启动基址偏移探测（旧实例占端口会暴露自己）；Windows Job Object 根治 |
| macOS Sparkle 等本机代理干扰测试请求 | 测试请求显式经 `Proxy::all("http://127.0.0.1:{port}")`，reqwest `.no_proxy()` 不读系统代理；与现有实测方法论一致 |
| 批量期间订阅刷新导致 tag 失配 | 快照语义：生成时刻锁定；tag 失配节点记 Failed 不 crash |
| 同出口多节点（机场同服务器挂多协议端口）致 ip-api 单 IP 429 | 该场景单出口请求数 = 挂靠节点数，通常 < 45/min；触限时 429 容错路径接管（freeipapi 单发重试，isProxy 顶替 hosting 语义），IP 字段缺失不阻塞结果 |

---

## 验证计划（按项目验证工作流）

1. **cargo test**：N-1 生成器单测（分片/过滤/tag 隔离/路由映射断言）
2. **sing-box check 实测**：32 节点批次生成配置零告警（1.14.0 二进制）
3. **live 双实例验证**：主实例 + test-core 并存运行；检测期间每秒采样主 ClashAPI `proxies.proxy.now` 全程不变
4. **取消路径**：批次中段取消 → test-core 被杀、端口释放、无残留（`lsof` 验证）
5. **崩溃注入**：手动 kill test-core → 调度器感知（探测失败聚集）→ 降级或终止批次，主实例无恙
6. **退出钩子**：批量进行中直接退出应用 → 进程清理（Activity Monitor / tasklist 验证）
7. **vue-tsc / vite build / vitest**：前端零改动仍跑全套（防意外回归）

---

## 明确不做

- ❌ 吞吐测速默认并发化（数值失真，见 N-5 决策点）
- ❌ test-core 接 ClashAPI（无控制面需求）
- ❌ test-core 持久化/cache（短命进程，无状态）
- ❌ 常驻"检测服务"形态（按需拉起按需销毁，避免常驻资源）
