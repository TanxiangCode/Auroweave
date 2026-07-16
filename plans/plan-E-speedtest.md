# 模块 E — 智能测速与负载均衡

> 状态：⏳ 待开始 | 优先级：🟡 中 | 前置：模块 B、C

---

## 目标

提供三档测速能力（延迟 / 单节点吞吐量 / 批量吞吐量），测速结果驱动节点卡片 UI，并接入负载均衡出站。

**验收标准**：
- 单节点测速结果误差在主流工具（Speedtest.net）可接受范围内（±20%）
- 批量测速前明确提示预计耗时与流量消耗
- 批量测速串行执行，不出现带宽互相干扰

---

## 任务清单

### E-1 延迟测速（Latency Test）

**文件**：`src-tauri/src/speedtest/latency.rs`

底层：复用 sing-box `urltest` 出站机制
- [ ] 对指定节点列表批量触发 urltest（通过 ClashAPI `GET /proxies/{name}` 中 `history` 字段读取）
- [ ] 手动触发 urltest：`PUT /proxies/{group}/` 触发延迟检测
- [ ] 结果写回 `proxy.store.ts` 的 `nodeMap`

---

### E-2 单节点吞吐量测速引擎

**文件**：`src-tauri/src/speedtest/throughput.rs`

**实现原理**：
通过对应出站代理（临时修改 sing-box 的 `selector.now`），向预设测速服务器发起 HTTP 分块下载/上传请求，限定时长（8s），计算平均速率。

```rust
pub async fn run_single_speed_test(
    node_tag: &str,
    test_url: &str,
    duration_secs: u64,
) -> Result<ThroughputResult, AppError> {
    // 1. 临时切换 selector 到目标节点
    // 2. 发起下载请求（通过本地 proxy: http://127.0.0.1:7890）
    // 3. 计时限定，统计接收字节数 → 计算 bps
    // 4. 恢复原来的 selector
}
```

- [ ] 下载测速：HTTP GET 分块流式接收，累计字节数 / 测试时长
- [ ] 上传测速：HTTP POST 随机数据块，累计发送字节数 / 测试时长
- [ ] 错误处理：节点不可达、测速服务器被封锁（尝试下一个备选 URL）
- [ ] 测速结果本地缓存（写入 Pinia store + 持久化 JSON）

---

### E-3 批量测速串行调度器

**文件**：`src-tauri/src/speedtest/scheduler.rs`

- [ ] 接收节点列表，**串行**逐个测速（禁止并发，避免带宽互相干扰）
- [ ] 每个节点测速完成后，通过 Tauri Event 推送进度到前端
- [ ] 支持取消（通过 `tokio_util::CancellationToken`）
- [ ] 批量测速前估算：节点数 × 单次测速耗时（8s）= 预计总耗时

**前端二次确认**：
```
⚠️ 批量测速确认

将对 「香港节点组」 的 12 个节点依次进行测速。

预计耗时：约 2 分钟
预计流量消耗：约 240 MB

[取消]  [开始测速]
```

---

### E-4 IPC 命令完整实现

**文件**：`src-tauri/src/commands/speedtest.rs`

- [ ] `speedtest_run_latency(node_tags: Vec<String>)` — 触发延迟测速
- [ ] `speedtest_run_single(node_tag: String)` — 单节点吞吐量测速（异步，返回结果）
- [ ] `speedtest_run_batch(group_tag: String)` — 批量测速（开始后立即返回，结果通过 Event 推送）
- [ ] `speedtest_cancel_batch()` — 取消批量测速
- [ ] `speedtest_get_results()` — 获取所有缓存结果

---

### E-5 节点卡片测速 UI

**文件**：`src/components/proxy/NodeCard.vue`

- [ ] 节点行右侧：独立的「⚡延迟」和「📶测速」两个按钮
- [ ] 延迟按钮：点击触发单节点延迟测速，显示 ms 数值（颜色编码：绿<100 / 黄100-300 / 红>300）
- [ ] 测速按钮：点击触发吞吐量测速，测速中显示流体色彩进度动画
- [ ] 测速完成：Ping / 下载 / 上传 三个数值从 0 滚动到最终值（动效与能量核视觉语言统一）
- [ ] 批量测速确认弹窗（磨砂玻璃卡片，非原生 alert）

---

### E-6 订阅导入自动分组

**文件**：`src-tauri/src/commands/subscription.rs`（扩展）

- [ ] 导入时解析节点名称中的地区标记
- [ ] 自动按地区创建 `urltest` 分组
- [ ] 「自动整理分组」开关（默认开启，Settings → SubscriptionPanel）

---

### E-7 负载均衡出站

**文件**：`src-tauri/src/core/config_builder.rs`（扩展）

- [ ] 支持生成 `loadbalance` 类型出站
- [ ] 默认关闭，Settings → RouteModePanel 中提供开关
- [ ] 开启前弹窗：「负载均衡模式下 IP 会在多个节点间跳变，可能影响需要固定 IP 的服务（如银行、流媒体）」
- [ ] 兼容性测试：验证 sing-box 1.14.0 的 `loadbalance` 出站 JSON 格式（参考模块 A-2）

---

### E-8 测速服务器设置项

**文件**：`src/views/panels/AdvancedPanel.vue`（模块 H 开发时实现，此处预留接口）

- 在 `AppSettings.speed_test_urls` 中存储自定义测速 URL 列表
- 默认值：`["https://speed.cloudflare.com/__down?bytes=10000000", "https://fast.com"]`

---

## 测速数据结构（前后端对齐）

```typescript
// src/types/index.ts（已定义）
interface ThroughputResult {
  download_bps: number;  // bytes/s
  upload_bps: number;
  tested_at: number;     // Unix timestamp (ms)
}
```

```rust
// src-tauri/src/commands/speedtest.rs（已定义骨架）
pub struct ThroughputResult {
    pub download_bps: u64,
    pub upload_bps: u64,
    pub tested_at: i64,
}
```

---

## 提交节点

```
feat(speedtest): complete speed test engine and node card UI (模块E)
```
