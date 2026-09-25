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

/// 单批次节点上限（同时占用的端口数）：批次越大进程重启越少，但单配置
/// 出站数过多会拖慢 sing-box 启动（去重/校验随节点数线性）。32 为经验平衡值。
pub const TEST_CORE_BATCH_SIZE: usize = 32;

/// 默认端口基址（settings.test_core_port_base 可覆盖）
pub const DEFAULT_PORT_BASE: u16 = 40040;

/// test-core 全局互斥锁（解锁检测/吞吐测速两调度器共享）：
/// 不同调度器各自 new 的 TestCoreManager 并发 spawn 会端口打架（同一基址
/// 段），任何 spawn 前必须持有此锁；guard 跨 await 由调用方生命周期保证。
static TEST_CORE_GLOBAL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 获取全局互斥 guard（批量调度器整批生命周期持有）
pub async fn acquire_global_lock() -> tokio::sync::MutexGuard<'static, ()> {
    TEST_CORE_GLOBAL_LOCK.lock().await
}

/// 单节点检测专用基址：批量走 test_core_port_base，单节点错开 500 端口段，
/// 与批量端口段错开；调用方仍必须持有 test-core 全局锁保护共享配置文件。
pub fn single_node_port_base(port_base: u16) -> u16 {
    port_base.wrapping_add(500)
}

// ============================================================
// N-1 测试配置生成
// ============================================================

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
/// 显式 `default_domain_resolver: local`：1.14 强制迁移项，缺失直接 FATAL
/// （见 singbox 1.14 升级实测记录）。无 TUN / clash_api / cache_file：
/// 短命测试实例零状态零控制面，与主实例端口/文件完全隔离。
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
        outbounds.push(raw);
        rules.push(json!({
            "inbound": [format!("t-in-{i}")],
            "outbound": format!("t-node-{i}"),
        }));
    }

    // direct 兜底出站：mixed 入站的非代理流量语义安全（本配置所有流量均被
    // inbound 规则钉死，final 仅在规则异常未命中时兜底）
    outbounds.push(json!({ "type": "direct", "tag": "direct" }));

    Ok(json!({
        // 探测是海量短连接，info 级会刷日志；warn 足够暴露配置错误
        "log": { "level": "warn" },
        // 极简 DNS：仅提供 default_domain_resolver 引用的 local 解析器
        // （实测 1.14.0：无 dns 段时 resolver "local" 不存在，启动直接 FATAL）。
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
            "default_domain_resolver": "local",
            "rules": rules,
            "final": "direct",
            "auto_detect_interface": true
        }
    }))
}

/// 测试配置文件路径（config_test.json，与主配置同目录便于相对定位二进制）
pub fn test_config_path() -> PathBuf {
    crate::get_config_dir().join("config_test.json")
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
                inner: Arc::new(Mutex::new(TestCoreInner { child: None, status: TestCoreStatus::Idle })),
                lifecycle_lock: Arc::new(Mutex::new(())),
                job,
            }
        }
        #[cfg(not(target_os = "windows"))]
        Self {
            inner: Arc::new(Mutex::new(TestCoreInner { child: None, status: TestCoreStatus::Idle })),
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
        let config = build_test_config(nodes, effective_base)?;
        let path = test_config_path();
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
            };
        }

        info!("[test-core] 测试内核就绪: {} 节点 @ 端口 {}..{}", count, effective_base, effective_base + count as u16 - 1);
        Ok(effective_base)
    }

    /// 停止测试内核（幂等）：kill + wait(3s) + 删配置文件
    pub async fn stop(&self) {
        let _lifecycle = self.lifecycle_lock.lock().await;
        let child = {
            let mut inner = self.inner.lock().await;
            inner.status = TestCoreStatus::Idle;
            inner.child.take()
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
        let _ = std::fs::remove_file(test_config_path());
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

        // 1.14 强制项：default_domain_resolver 缺失即 FATAL；实测还需 dns.servers
        // 提供 "local" 解析器（无 dns 段时 resolver 不存在，见上）
        assert_eq!(cfg["route"]["default_domain_resolver"], json!("local"));
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
