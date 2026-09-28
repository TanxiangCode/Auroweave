/// 测试内核实例（test-core）— 配置生成器与进程管理
/// 作者: TanXiang
///
/// 设计（plans/plan-N-test-core.md）：批量检测/测速时拉起短生命周期的第二 sing-box
/// 进程，N 入站 + N 出站 + N 条 inbound 路由规则——每个被测节点一个专属本地端口，
/// 流量被规则钉死到对应节点出站。主实例、主 selector、用户流量全程零打扰。
///
/// 与主实例的隔离：独立配置文件（结束即删）、端口基址 40040+、不启 ClashAPI/
/// cache_file、outbound tag 加 t- 前缀。文档依据（v1.14.0）：
/// route/rule.md "inbound" 匹配器；mixed 入站多实例天然支持。
use crate::error::AppError;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Child;
use tokio::sync::Mutex;
use log::{info, warn};

/// 端口布局（三段互不重叠，各自独立成环）
///
/// 探测面（常驻）与吞吐测速（短命）历史上共用 40040 基址：探测面一旦常驻，
/// 吞吐测速再在同一段起进程必然撞端口。改为三段隔离后，探测面常驻不再阻塞
/// 吞吐测速，二者可并存（各自独立生命周期，无需互斥等待）。
///
/// 段边界与节点规模：探测面段容量 = SINGLE_NODE_PORT_BASE - PROBE_PLANE_PORT_BASE
/// = 2000，远超任何现实订阅规模；`probe_plane::spawn` 另有显式越界校验。
pub const PROBE_PLANE_PORT_BASE: u16 = 40040;
pub const THROUGHPUT_PORT_BASE: u16 = 41040;
pub const SINGLE_NODE_PORT_BASE: u16 = 42040;

/// 单批次节点上限（吞吐测速路径的出站/入站对数）。
///
/// 原值 32 的理由是注释所述「单配置出站数过多会拖慢 sing-box 启动」，该假设已
/// 被实测推翻（2026-09-28，sing-box 1.14.2 / macOS-arm64，370 节点真实订阅）：
///
/// ```text
/// $ sing-box check -c <372 inbounds + 373 outbounds + 372 rules>
/// real 0m0.061s
/// $ sing-box run   -c <同上>
/// 372 个端口全部 LISTEN，就绪 < 100ms
/// ```
///
/// 启动开销对节点数近线性且常数极小，远小于「每 32 个重启一次进程」的代价
/// ——370 节点在旧配置下要拉起 12 次进程、经历 12 轮「最多 3s 就绪」上限。
/// 现默认一次装下全部节点（受段容量 2000 约束，见上）。
pub const TEST_CORE_BATCH_SIZE: usize = 480;

/// 默认端口基址（settings.test_core_port_base 可覆盖；作用于吞吐测速段）
pub const DEFAULT_PORT_BASE: u16 = THROUGHPUT_PORT_BASE;

/// 单节点检测专用基址：与吞吐测速段错开 1000 端口。
/// 调用方仍必须持有 test-core 全局锁保护共享配置文件。
pub fn single_node_port_base(port_base: u16) -> u16 {
    if port_base == THROUGHPUT_PORT_BASE {
        SINGLE_NODE_PORT_BASE
    } else {
        port_base.wrapping_add(1000)
    }
}

/// 探测面段可容纳的最大节点数（端口号连续占用，1 节点 = 1 端口）
pub const PROBE_PLANE_MAX_NODES: usize =
    (SINGLE_NODE_PORT_BASE - PROBE_PLANE_PORT_BASE) as usize;

/// test-core 全局互斥锁（解锁检测/吞吐测速两调度器共享）：
/// 不同调度器各自 new 的 TestCoreManager 并发 spawn 会端口打架（同一基址
/// 段），任何 spawn 前必须持有此锁；guard 跨 await 由调用方生命周期保证。
static TEST_CORE_GLOBAL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 获取全局互斥 guard（批量调度器整批生命周期持有）
pub async fn acquire_global_lock() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_CORE_GLOBAL_LOCK.lock().await
}

// ============================================================
// N-1 测试配置生成
// ============================================================

/// 节点集合指纹（FNV-1a 64，tag 排序后计算）
///
/// 用于探测面判断「常驻实例是否已覆盖当前节点集」：订阅未变时直接复用常驻
/// 进程（零重启），变了才重建。顺序无关——传入顺序不同但集合相同则指纹相同。
pub fn node_fingerprint(nodes: &[crate::core::parser::ParsedOutbound]) -> u64 {
    let mut tags: Vec<&str> = nodes.iter().map(|n| n.tag.as_str()).collect();
    tags.sort_unstable();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for t in tags {
        for b in t.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        h ^= 0xff; // tag 边界分隔，避免 "ab"+"c" 与 "a"+"bc" 同指纹
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

/// 节点索引分片（32/批；返回原节点列表的下标分批，避免克隆 raw_json）
pub fn plan_batches(len: usize, batch_size: usize) -> Vec<Vec<usize>> {
    let batch_size = batch_size.max(1);
    (0..len)
        .collect::<Vec<usize>>()
        .chunks(batch_size)
        .map(|c| c.to_vec())
        .collect()
}

/// 生成单批次测试配置：
/// - inbound i：`t-in-{i}` mixed，127.0.0.1:{port_base+i}
/// - outbound i：节点 raw_json 克隆 + tag 改写为 `t-node-{i}`（与主配置 tag 空间隔离，
///   规避订阅刷新双写问题——test-core 消费的是生成时刻快照）
/// - route：`inbound: t-in-{i} → outbound: t-node-{i}` 逐条钉死
///
/// 每个出站显式携带 `domain_resolver: local`：1.14 对「server 为域名且无解析器」
/// 的出站直接 FATAL（见 singbox 1.14 升级实测记录）。P0 修复刻意**不**使用
/// `route.default_domain_resolver`——该字段会让出站解析绕过 dns.rules。
/// 无 TUN / clash_api / cache_file：短命测试实例零状态零控制面，
/// 与主实例端口/文件完全隔离。
pub fn build_test_config(
    nodes: &[crate::core::parser::ParsedOutbound],
    port_base: u16,
) -> Result<Value, AppError> {
    let mut inbounds = Vec::with_capacity(nodes.len());
    let mut outbounds = Vec::with_capacity(nodes.len() + 1);
    let mut rules = Vec::with_capacity(nodes.len());

    for (i, node) in nodes.iter().enumerate() {
        inbounds.push(json!({
            "type": "mixed",
            "tag": format!("t-in-{i}"),
            "listen": "127.0.0.1",
            "listen_port": port_base + i as u16,
        }));
        let mut raw = node.raw_json.clone();
        raw["tag"] = json!(format!("t-node-{i}"));
        // P0 修复（与主配置同源）：显式绑定 domain_resolver。
        // 1.14 中 route.default_domain_resolver 一旦指定 tag 会绕过 dns.rules，
        // 而此处测试实例的 dns 段只有一个 local server，指定它没有收益，
        // 反而让"去掉该字段"这条更干净的路径不可行。因此这里给每个节点
        // 显式绑定，direct 同样显式绑定——两者都不依赖 default 字段。
        raw["domain_resolver"] = json!("local");
        outbounds.push(raw);
        rules.push(json!({
            "inbound": [format!("t-in-{i}")],
            "outbound": format!("t-node-{i}"),
        }));
    }

    // direct 兜底出站：mixed 入站的非代理流量语义安全（本配置所有流量均被
    // inbound 规则钉死，final 仅在规则异常未命中时兜底）
    outbounds.push(json!({ "type": "direct", "tag": "direct", "domain_resolver": "local" }));

    Ok(json!({
        // 探测是海量短连接，info 级会刷日志；warn 足够暴露配置错误
        "log": { "level": "warn" },
        // 极简 DNS：仅提供各出站 domain_resolver 引用的 local 解析器
        // （实测：无 dns 段时 resolver "local" 不存在，启动直接 FATAL）。
        // 节点域名由远端解析（代理协议 CONNECT 主机名透传），本地解析器
        // 只兜底规则/直连出站的域名字段。
        "dns": {
            "servers": [
                { "tag": "local", "type": "local" }
            ],
            "final": "local"
        },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "route": {
            // P0 修复：不再需要 default_domain_resolver——每个出站已自带
            // domain_resolver（见上方 outbounds 构造）。留着它反而会让
            // 出站解析固定绑定、绕过 dns.rules。
            "rules": rules,
            "final": "direct",
            "auto_detect_interface": true
        }
    }))
}

/// 测试配置文件路径（与主配置同目录便于相对定位二进制）
///
/// 按端口段分文件：探测面常驻、吞吐测速短命，若共用同一文件，两者同时存在时
/// 后写者会覆盖前者配置，被覆盖一方随后重启即加载到错误的节点集。
pub fn test_config_path() -> PathBuf {
    test_config_path_for(THROUGHPUT_PORT_BASE)
}

/// 指定端口段对应的测试配置文件路径
pub fn test_config_path_for(port_base: u16) -> PathBuf {
    crate::get_config_dir().join(format!("config_test_{port_base}.json"))
}

/// 批次端口可用性预检：TcpListener 试占全部端口后立即释放
/// （SO_REUSEADDR 语义由 OS 保证 listen→drop→sing-box bind 的时间窗口极短）
pub fn check_ports_free(port_base: u16, count: usize) -> bool {
    let mut held: Vec<std::net::TcpListener> = Vec::with_capacity(count);
    for i in 0..count {
        match std::net::TcpListener::bind(("127.0.0.1", port_base + i as u16)) {
            Ok(l) => held.push(l),
            Err(_) => return false,
        }
    }
    drop(held); // 全部占得住才认为可用，统一释放
    true
}

// ============================================================
// N-2 进程管理
// ============================================================

/// test-core 进程状态
#[derive(Debug, Clone, PartialEq)]
pub enum TestCoreStatus {
    Idle,
    Running { port_base: u16, node_count: usize },
}

/// 测试内核进程管理器（与 SidecarManager 完全解耦：互不知晓、互不干扰）
///
/// 不复用 SidecarManager::start：其状态机/ClashAPI 就绪探测/退出 watchdog/
/// SUID 特权链均为主实例语义；测试实例只需要"端口通了吗"。
pub struct TestCoreManager {
    inner: Arc<Mutex<TestCoreInner>>,
    /// 生命周期串行锁：并发 spawn 产生孤儿进程的防御（同 SidecarManager 语义）
    lifecycle_lock: Arc<Mutex<()>>,
    #[cfg(target_os = "windows")]
    job: Option<crate::system::job::JobObject>,
}

struct TestCoreInner {
    child: Option<Child>,
    status: TestCoreStatus,
    /// 常驻实例消费的节点集指纹（Idle 时为 0）；`ensure_running` 据此判定
    /// 「节点集未变 → 复用常驻进程，零重启」
    fingerprint: u64,
}

impl TestCoreManager {
    pub fn new() -> Self {
        // Windows Job Object 句柄随 manager 常驻：KILL_ON_JOB_CLOSE 保证
        // 主进程退出时测试内核随之销毁（Exit 钩子为第二道保险）
        #[cfg(target_os = "windows")]
        {
            let job = crate::system::job::JobObject::create()
                .map_err(|e| warn!("[test-core] 创建 Job Object 失败（退出兜底退化为 Exit 钩子）: {}", e))
                .ok();
            Self {
                inner: Arc::new(Mutex::new(TestCoreInner { child: None, status: TestCoreStatus::Idle, fingerprint: 0 })),
                lifecycle_lock: Arc::new(Mutex::new(())),
                job,
            }
        }
        #[cfg(not(target_os = "windows"))]
        Self {
            inner: Arc::new(Mutex::new(TestCoreInner { child: None, status: TestCoreStatus::Idle, fingerprint: 0 })),
            lifecycle_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 拉起测试内核并等待就绪
    ///
    /// 返回实际使用的端口基址（调用方预检冲突时可能已偏移）。
    /// 就绪判据：首个端口 TCP 可连（裸 TCP connect，无 HTTP 语义——
    /// mixed 入站 accept 即监听就绪）。
    pub async fn spawn(
        &self,
        nodes: &[crate::core::parser::ParsedOutbound],
        port_base: u16,
    ) -> Result<u16, AppError> {
        let _lifecycle = self.lifecycle_lock.lock().await;

        // 串行锁内幂等：已有实例先停（正常不会发生——调度器单实例锁在先）
        {
            let inner = self.inner.lock().await;
            if inner.status != TestCoreStatus::Idle {
                return Err(AppError::Sidecar("测试内核已在运行，需先停止".to_string()));
            }
        }

        if nodes.is_empty() {
            return Err(AppError::Validation("测试节点列表为空".to_string()));
        }

        // 端口预检：冲突整体 +1000 重试（最多 3 次），避免与主配置/其他应用撞口
        let count = nodes.len();
        let mut effective_base = port_base;
        let mut attempts = 0;
        loop {
            if check_ports_free(effective_base, count) {
                break;
            }
            attempts += 1;
            if attempts >= 3 {
                return Err(AppError::Sidecar(format!(
                    "测试端口 {}..{} 被占用（已尝试偏移 {} 次）",
                    effective_base,
                    effective_base + count as u16 - 1,
                    attempts
                )));
            }
            effective_base += 1000;
        }

        // 生成 + 原子写配置（复用 fs_utils；写盘在 async 上下文但量小可忽略）
        // 按端口段分文件：探测面常驻时不被短命的吞吐测速覆盖
        let config = build_test_config(nodes, effective_base)?;
        let path = test_config_path_for(effective_base);
        crate::fs_utils::atomic_write(&path, &serde_json::to_vec_pretty(&config).unwrap_or_default())
            .map_err(|e| AppError::Io(format!("写入测试配置失败: {}", e)))?;

        // 拉起进程（stderr null：level=warn 无诊断需求，stdout null）
        let binary = crate::core::sidecar::SidecarManager::resolve_binary_path()?;
        info!("[test-core] 启动测试内核: {:?} run -c {:?}（{} 节点，端口 {}..）",
            binary, path, count, effective_base);

        let mut child = tokio::process::Command::new(&binary)
            .arg("run")
            .arg("-c")
            .arg(&path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| AppError::Sidecar(format!("拉起测试内核失败: {}", e)))?;

        // Windows：绑定常驻 Job Object（KILL_ON_JOB_CLOSE——句柄由 manager 持有，
        // 主进程退出即销毁测试内核；spawn 失败仅告警，Exit 钩子兜底）
        #[cfg(target_os = "windows")]
        if let (Some(j), Some(h)) = (self.job.as_ref(), child.raw_handle()) {
            if let Err(e) = j.assign_process(h) {
                warn!("[test-core] 测试内核加入 Job Object 失败: {}", e);
            }
        }

        // 就绪探测：首个端口可连即就绪；上限 3s；500ms 内退出 = 配置错误
        let ready = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if let Ok(Some(_)) = child.try_wait() {
                    return Err::<(), AppError>(AppError::Sidecar(
                        "测试内核启动后立即退出（配置校验失败或二进制异常）".to_string(),
                    ));
                }
                if port_connectable(effective_base).await {
                    return Ok::<(), AppError>(());
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        })
        .await;

        match ready {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                // 早期退出：回收进程 + 清配置，错误上抛触发降级
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = std::fs::remove_file(&path);
                return Err(e);
            }
            Err(_) => {
                // 3s 未就绪：杀掉回收，避免半启动残留
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = std::fs::remove_file(&path);
                return Err(AppError::Sidecar("测试内核 3 秒内未就绪".to_string()));
            }
        }

        {
            let mut inner = self.inner.lock().await;
            // 防御性回收（同 SidecarManager 语义）
            if let Some(mut stale) = inner.child.take() {
                let _ = stale.kill().await;
                let _ = stale.wait().await;
            }
            *inner = TestCoreInner {
                child: Some(child),
                status: TestCoreStatus::Running { port_base: effective_base, node_count: count },
                fingerprint: node_fingerprint(nodes),
            };
        }

        info!("[test-core] 测试内核就绪: {} 节点 @ 端口 {}..{}", count, effective_base, effective_base + count as u16 - 1);
        Ok(effective_base)
    }

    /// 常驻语义拉取：节点集未变则复用现有进程，变了才重建
    ///
    /// 探测面在应用生命周期内常驻，每次调度 tick 都可能调用本方法。订阅未刷新
    /// 时节点集指纹不变 → 直接返回既有基址，**零进程重启**（这是常驻架构相对
    /// 「每批重启」的核心收益：370 节点原本每轮要拉起 12 次进程）。
    ///
    /// 返回实际使用的端口基址；调用方按 `base + index` 映射节点端口。
    pub async fn ensure_running(
        &self,
        nodes: &[crate::core::parser::ParsedOutbound],
        port_base: u16,
    ) -> Result<u16, AppError> {
        let fp = node_fingerprint(nodes);
        {
            let inner = self.inner.lock().await;
            if let TestCoreStatus::Running { port_base: base, node_count } = inner.status {
                if inner.fingerprint == fp && node_count == nodes.len() {
                    return Ok(base);
                }
            }
        }
        // 节点集变了（或进程已死）：先回收再重建
        self.stop().await;
        self.spawn(nodes, port_base).await
    }

    /// 停止测试内核（幂等）：kill + wait(3s) + 删配置文件
    pub async fn stop(&self) {
        let _lifecycle = self.lifecycle_lock.lock().await;
        let (child, used_base) = {
            let mut inner = self.inner.lock().await;
            let used_base = match inner.status {
                TestCoreStatus::Running { port_base, .. } => Some(port_base),
                TestCoreStatus::Idle => None,
            };
            inner.status = TestCoreStatus::Idle;
            inner.fingerprint = 0;
            (inner.child.take(), used_base)
        };
        if let Some(mut child) = child {
            if let Err(e) = child.kill().await {
                warn!("[test-core] kill 测试内核失败: {}", e);
            }
            match tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await {
                Ok(_) => info!("[test-core] 测试内核已停止并回收"),
                Err(_) => warn!("[test-core] 测试内核 3 秒内未退出，进程可能残留"),
            }
        }
        // 只删本实例实际写过的那个段的文件：误删另一段（探测面常驻）的配置
        // 会让对方下次重启前配置缺失
        if let Some(base) = used_base {
            let _ = std::fs::remove_file(test_config_path_for(base));
        }
    }
}

/// 裸 TCP 可连性探测（mixed 入站 accept 即就绪，无 HTTP 握手必要）
async fn port_connectable(port: u16) -> bool {
    tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok()
}

impl Default for TestCoreManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 单元测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_node(tag: &str, server: &str) -> crate::core::parser::ParsedOutbound {
        crate::core::parser::ParsedOutbound {
            tag: tag.to_string(),
            r#type: "shadowsocks".to_string(),
            server: Some(server.to_string()),
            server_port: Some(8388),
            raw_json: serde_json::json!({
                "type": "shadowsocks",
                "tag": tag,
                "server": server,
                "server_port": 8388,
                "method": "aes-128-gcm",
                "password": "test"
            }),
        }
    }

    #[test]
    fn port_segments_are_disjoint_and_ordered() {
        // 探测面 / 吞吐 / 单节点三段必须互不重叠且升序——重叠会导致常驻探测面
        // 与短命吞吐测速抢同一批端口（实测 370 节点时表现为后者起不来）
        assert!(PROBE_PLANE_PORT_BASE < THROUGHPUT_PORT_BASE);
        assert!(THROUGHPUT_PORT_BASE < SINGLE_NODE_PORT_BASE);
        // 探测面段容量须覆盖现实订阅规模（本项目实测 370~380 节点）
        assert!(
            PROBE_PLANE_MAX_NODES >= 1000,
            "探测面段容量 {} 过小，370 节点订阅会越界",
            PROBE_PLANE_MAX_NODES
        );
        // 单节点基址在默认吞吐基址下映射到独立段，不落在探测面段内
        let single = single_node_port_base(DEFAULT_PORT_BASE);
        assert!(single >= SINGLE_NODE_PORT_BASE);
        assert!(
            (PROBE_PLANE_PORT_BASE..PROBE_PLANE_PORT_BASE + PROBE_PLANE_MAX_NODES as u16)
                .all(|p| p != single),
            "单节点端口不得落入探测面段"
        );
    }

    #[test]
    fn single_batch_covers_realistic_pool_without_restart() {
        // 370 节点真实订阅：旧配置（batch=32）要拉起 12 次进程；
        // 现配置一次装下 → 常驻架构每轮零重启
        let plan = plan_batches(370, TEST_CORE_BATCH_SIZE);
        assert_eq!(plan.len(), 1, "370 节点应落在单个批次内");
        assert_eq!(plan[0].len(), 370);
    }

    #[test]
    fn fingerprint_is_order_independent_and_change_sensitive() {
        let a = mock_node("HK-01", "a.example.com");
        let b = mock_node("JP-01", "b.example.com");
        let c = mock_node("US-01", "c.example.com");

        // 顺序无关：ensure_running 据此复用常驻进程
        assert_eq!(
            node_fingerprint(&[a.clone(), b.clone(), c.clone()]),
            node_fingerprint(&[c.clone(), a.clone(), b.clone()])
        );
        // 集合变化 → 指纹变化（订阅刷新后必须重建）
        assert_ne!(
            node_fingerprint(&[a.clone(), b.clone()]),
            node_fingerprint(&[a.clone(), b.clone(), c.clone()])
        );
        // tag 是身份：同名节点即便 server 不同也视为同一节点（订阅原地更新参数
        // 不应触发探测面重建——那是刷新路径显式 stop 的职责）
        assert_eq!(
            node_fingerprint(&[a.clone()]),
            node_fingerprint(&[mock_node("HK-01", "other.example.com")])
        );
        // 空集有稳定指纹（非 0，避免与 Idle 的 0 哨兵混淆）
        assert_ne!(node_fingerprint(&[]), 0);
    }

    #[test]
    fn fingerprint_does_not_conflate_tag_boundaries() {
        // 无分隔符拼接会让 "ab"+"c" 与 "a"+"bc" 同指纹 → 误判常驻实例可复用
        let ab_c = vec![mock_node("ab", "x.example.com"), mock_node("c", "y.example.com")];
        let a_bc = vec![mock_node("a", "x.example.com"), mock_node("bc", "y.example.com")];
        assert_ne!(node_fingerprint(&ab_c), node_fingerprint(&a_bc));
    }

    #[test]
    fn test_config_paths_are_per_segment() {
        // 常驻探测面与短命吞吐测速必须写不同文件，否则互相覆盖节点集
        let probe = test_config_path_for(PROBE_PLANE_PORT_BASE);
        let tp = test_config_path_for(THROUGHPUT_PORT_BASE);
        assert_ne!(probe, tp);
        assert_eq!(test_config_path(), tp, "默认路径应指向吞吐测速段");
    }

    #[test]
    fn batch_plan_splits_by_32() {
        assert_eq!(plan_batches(0, 32), Vec::<Vec<usize>>::new());
        assert_eq!(plan_batches(5, 32), vec![vec![0, 1, 2, 3, 4]]);
        let plan = plan_batches(70, 32);
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].len(), 32);
        assert_eq!(plan[1].len(), 32);
        assert_eq!(plan[2].len(), 6);
        // 全量覆盖且有序
        let flat: Vec<usize> = plan.concat();
        assert_eq!(flat, (0..70).collect::<Vec<usize>>());
        // batch_size=0 防御性回退为 1
        assert_eq!(plan_batches(2, 0), vec![vec![0], vec![1]]);
    }

    #[test]
    fn config_structure_maps_ports_to_nodes() {
        let nodes: Vec<_> = (0..3)
            .map(|i| mock_node(&format!("节点{i}"), &format!("srv{i}.example.com")))
            .collect();
        let cfg = build_test_config(&nodes, 40040).unwrap();

        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 3);
        assert_eq!(inbounds[0]["listen_port"], json!(40040));
        assert_eq!(inbounds[2]["listen_port"], json!(40042));
        assert_eq!(inbounds[0]["tag"], json!("t-in-0"));

        let outbounds = cfg["outbounds"].as_array().unwrap();
        // N 节点 + direct 兜底
        assert_eq!(outbounds.len(), 4);
        assert_eq!(outbounds[0]["tag"], json!("t-node-0"));
        assert_eq!(outbounds[3]["tag"], json!("direct"));
        // raw_json 完整保留（server 字段透传）
        assert_eq!(outbounds[1]["server"], json!("srv1.example.com"));

        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[1]["inbound"], json!(["t-in-1"]));
        assert_eq!(rules[1]["outbound"], json!("t-node-1"));

        // 1.14 强制项：server 为域名的出站若无 domain_resolver 即 FATAL。
        // P0 修复后改为每个出站自带解析器，route 上不再有 default_domain_resolver
        // （留着它会让出站解析绕过 dns.rules）。
        assert!(cfg["route"].get("default_domain_resolver").is_none());
        assert_eq!(outbounds[0]["domain_resolver"], json!("local"));
        assert_eq!(outbounds[3]["domain_resolver"], json!("local"));
        assert_eq!(cfg["dns"]["servers"][0]["type"], json!("local"));
        // 测试配置不启控制面/缓存（与主实例隔离）
        assert!(cfg.get("experimental").is_none());
    }

    #[test]
    fn tag_prefix_isolation_from_main_config() {
        // 主配置 tag 空间隔离：即使节点原名撞主组名（proxy/auto 等）也不冲突
        let nodes = vec![
            mock_node("proxy", "a.example.com"),
            mock_node("auto", "b.example.com"),
        ];
        let cfg = build_test_config(&nodes, 40040).unwrap();
        let outbounds = cfg["outbounds"].as_array().unwrap();
        assert_eq!(outbounds[0]["tag"], json!("t-node-0"));
        assert_eq!(outbounds[1]["tag"], json!("t-node-1"));
        assert!(outbounds.iter().all(|o| o["tag"].as_str().unwrap().starts_with("t-")
            || o["tag"] == json!("direct")));
    }

    #[test]
    fn port_conflict_detection() {
        // 预检函数语义：占用任一端口返回 false（本测试用已占用端口 1）
        // 端口 1 通常需要 root，macOS/Linux 上 bind 失败即占用语义成立；
        // 若环境允许绑定（罕见），跳过断言不误报
        if let Err(_) = std::net::TcpListener::bind(("127.0.0.1", 1)) {
            assert!(!check_ports_free(1, 2));
        }
        // 高位无冲突区间应返回 true（40040+ 常规空闲）
        if std::net::TcpListener::bind(("127.0.0.1", 49999)).is_ok() {
            assert!(check_ports_free(49999, 2));
        }
    }
}
