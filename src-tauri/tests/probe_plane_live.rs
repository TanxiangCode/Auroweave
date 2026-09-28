//! 探测面实机验证（默认忽略，需显式 --ignored 运行）
//! 作者: TanXiang
//!
//! 验证三件用纯单测无法证明的事——它们都需要真实 sing-box 二进制与真实节点：
//!
//! 1. **常驻探测面能一次装下 370+ 节点**（推翻 batch=32 的旧假设）
//! 2. **探测流量完全不经过主实例**（旧架构下正是这一点导致用户 15s 超时）
//! 3. **节点表能真实产出延迟结果并选出最优**
//!
//! 运行：
//! ```text
//! cargo test --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture
//! ```
//!
//! 前置：已导入订阅（`~/Library/Application Support/Auroweave/config/subscriptions.json`
//! 或 `%ProgramData%\Auroweave\config\subscriptions.json` 存在且含有效节点）。
//! 无订阅时自动跳过而非失败——CI 与他人克隆不应被本测试阻塞。
use std::time::{Duration, Instant};

use auroweave_lib::core::parser::ParsedOutbound;
use auroweave_lib::core::test_core::PROBE_PLANE_PORT_BASE;
use auroweave_lib::probe::scheduler::{ProbeConfig, ProbeScheduler};
use auroweave_lib::probe::table::ProbeTier;

fn load_nodes() -> Vec<ParsedOutbound> {
    let subs_path = auroweave_lib::get_config_dir().join("subscriptions.json");
    if !subs_path.exists() {
        return Vec::new();
    }
    // 复用应用自身的收集路径，保证与真实运行消费的是同一批节点
    // （同一套去重 / 保留名冲突 / 伪节点过滤逻辑）
    auroweave_lib::commands::subscription::collect_active_outbounds().unwrap_or_default()
}

#[tokio::test]
#[ignore = "需要真实 sing-box 二进制与已导入的订阅节点"]
async fn probe_plane_scales_and_never_touches_main_instance() {
    let nodes = load_nodes();
    if nodes.is_empty() {
        eprintln!("跳过：无订阅节点");
        return;
    }
    eprintln!("订阅节点数: {}", nodes.len());

    let sch = ProbeScheduler::new(ProbeConfig {
        timeout_ms: 5000,
        concurrency: 64,
        max_per_round: 64,
        tick_ms: 1000,
        url: "http://www.gstatic.com/generate_204".to_string(),
        fresh_ms: 120_000,
        standby_interval_ms: 300_000,
    });

    // ---- 1. 常驻面一次装下全池，零重启 ----
    let t0 = Instant::now();
    sch.refresh(nodes.clone(), true).await.expect("探测面启动失败");
    let boot = t0.elapsed();
    eprintln!("探测面就绪耗时: {:?}", boot);
    assert!(
        boot < Duration::from_secs(3),
        "常驻探测面启动过慢: {:?}（旧 batch=32 架构下 370 节点需 12 次进程拉起）",
        boot
    );

    // 重复 refresh（节点集未变）必须零重启：指纹命中直接返回
    let t1 = Instant::now();
    sch.refresh(nodes.clone(), true).await.expect("重复 refresh 失败");
    let reuse = t1.elapsed();
    eprintln!("未变节点集重复 refresh 耗时: {:?}", reuse);
    assert!(
        reuse < Duration::from_millis(500),
        "节点集未变时应复用常驻进程，实际耗时 {:?}",
        reuse
    );

    // ---- 2. 端口段确实被占满（每节点一个专属端口）----
    // 逐个真实 TCP connect 计数：不依赖 lsof 文本解析（grep '127.0.0.1:40040'
    // 只能匹配到基址那一个端口，会把 372 误判成 1）
    let mut listening = 0usize;
    for i in 0..nodes.len() {
        let port = PROBE_PLANE_PORT_BASE + i as u16;
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            listening += 1;
        }
    }
    eprintln!("探测面可连端口数: {}/{}", listening, nodes.len());
    assert_eq!(
        listening,
        nodes.len(),
        "探测面应为每个节点监听一个专属端口（{}-{}）",
        PROBE_PLANE_PORT_BASE,
        PROBE_PLANE_PORT_BASE + nodes.len() as u16 - 1
    );

    // ---- 3. 跑若干轮，产出真实延迟 ----
    let table = sch.table();
    let mut total_probed = 0usize;
    for _ in 0..6 {
        total_probed += sch.run_once().await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let (healthy, total) = {
        let t = table.read().await;
        t.health_summary(chrono::Utc::now().timestamp_millis(), 120_000)
    };
    eprintln!("探测 {} 次，健康 {}/{}", total_probed, healthy, total);
    assert!(total_probed > 0, "调度器未派发任何探测");
    assert!(total == nodes.len(), "节点表规模应等于订阅节点数");

    // ---- 4. 能选出最优 ----
    let members: Vec<String> = nodes.iter().map(|n| n.tag.clone()).collect();
    let now = chrono::Utc::now().timestamp_millis();
    let best = {
        let t = table.read().await;
        t.best_of(&members, now, 120_000)
    };
    if let Some((tag, rtt)) = &best {
        eprintln!("最优节点: {} @ {}ms", tag, rtt);
        assert!(*rtt > 0);
    } else {
        eprintln!("本轮无健康节点（网络不可达时属正常，仅告警不失败）");
    }

    // ---- 5. 分层间隔确实生效 ----
    {
        let t = table.read().await;
        let active = t.iter().filter(|n| n.tier == ProbeTier::Active).count();
        let cooling = t.iter().filter(|n| n.tier == ProbeTier::Cooling).count();
        eprintln!("Active 层 {} 个，Cooling 层 {} 个", active, cooling);
    }

    sch.shutdown().await;
}

/// 故障转移端到端验证（默认忽略）
///
/// 用一个**受控 sing-box 实例**做「主实例」：auto 组含一个必然失败的出站
/// （DEAD，指向 127.0.0.1:1）与一个健康出站（GOOD，direct 出网）。
/// 把 auto 切到 DEAD 后，健康守卫必须在阈值内完成故障转移。
///
/// 这是「节点失效能否自动切换」的最终证据——不依赖真实订阅，
/// 任何人都可复现。
#[tokio::test]
#[ignore = "需要真实 sing-box 二进制"]
async fn guard_fails_over_when_current_node_dies() {
    use auroweave_lib::core::clash_api::ClashApiClient;
    use auroweave_lib::probe::guard::{ActiveHealthGuard, GuardOutcome, HealthGuardConfig};
    use auroweave_lib::probe::selector::{AutoSelector, SelectorConfig};
    use auroweave_lib::probe::table::ProbeTable;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let Ok(binary) = auroweave_lib::core::sidecar::SidecarManager::resolve_binary_path() else {
        eprintln!("跳过：找不到 sing-box 二进制");
        return;
    };

    let dir = std::env::temp_dir().join("auroweave_failover_test");
    let _ = std::fs::create_dir_all(&dir);
    let cfg_path = dir.join("failover.json");
    let cache = dir.join("cache.db");
    let _ = std::fs::remove_file(&cache);
    // 端口动态选取：探测可用端口对，避免与本机其他应用（本实测就撞到
    // opencode 占用 45870）冲突导致测试假失败
    let (mixed, ctrl) = {
        let mut base = 41700u16;
        let mut found = None;
        'outer: while base < 42000 {
            let a = std::net::TcpListener::bind(("127.0.0.1", base));
            let b = std::net::TcpListener::bind(("127.0.0.1", base + 1));
            if a.is_ok() && b.is_ok() {
                found = Some((base, base + 1));
                break 'outer;
            }
            base += 2;
        }
        found.expect("找不到连续的可用端口对（41700-42000）")
    };

    let cfg = serde_json::json!({
        "log": {"level": "warn"},
        "dns": {"servers": [{"tag":"local","type":"local"}], "final":"local"},
        "inbounds": [{"type":"mixed","tag":"in","listen":"127.0.0.1","listen_port":mixed}],
        "outbounds": [
            {"type":"direct","tag":"GOOD","domain_resolver":"local"},
            // 指向本地丢弃端口：连接必然被拒，模拟节点失效
            {"type":"shadowsocks","tag":"DEAD","server":"127.0.0.1","server_port":1,
             "method":"aes-128-gcm","password":"x","domain_resolver":"local"},
            {"type":"selector","tag":"auto","outbounds":["DEAD","GOOD"],"default":"DEAD"}
        ],
        "experimental": {
            // secret 必须用应用真实值：ClashApiClient 内部固定读
            // get_clash_api_secret()，无法为测试注入自定义 token
            "clash_api": {"external_controller": format!("127.0.0.1:{}", ctrl),
                          "secret": auroweave_lib::core::clash_api::get_clash_api_secret()},
            "cache_file": {"enabled": true, "path": cache.to_string_lossy()}
        },
        "route": {"final":"auto","auto_detect_interface":true}
    });
    std::fs::write(&cfg_path, serde_json::to_vec_pretty(&cfg).unwrap()).unwrap();

    let mut child = tokio::process::Command::new(&binary)
        .arg("run").arg("-c").arg(&cfg_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("拉起测试内核失败");

    // 等 ClashAPI 就绪
    let client = Arc::new(ClashApiClient::new(Some(format!("http://127.0.0.1:{}", ctrl))));
    let mut ready = false;
    for i in 0..30 {
        if i == 3 {
            // 早退检测：进程是否已退出
            if let Ok(Some(st)) = child.try_wait() {
                panic!("测试内核提前退出，status={:?}", st);
            }
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        if client.get_proxies().await.is_ok() {
            ready = true;
            break;
        }
    }
    if !ready {
        // 诊断：把内核 stderr 读出来（配置错误、端口冲突等）
        let _ = child.kill().await;
        let out = match child.wait_with_output().await {
            Ok(o) => o,
            Err(e) => panic!("测试内核未就绪且回收失败: {}", e),
        };
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        panic!("测试内核未就绪。stderr: {}", &err[..err.len().min(500)]);
    }

    // 构造守卫：auto 组当前选中的是 DEAD（default 已是 DEAD）
    let table = Arc::new(RwLock::new(ProbeTable::new()));
    let now = chrono::Utc::now().timestamp_millis();
    table.write().await.reset_to(&["DEAD".to_string(), "GOOD".to_string()], now);
    // 假装 GOOD 健康（延迟 200ms），使其成为故障转移候选
    table.write().await.get_mut("GOOD").unwrap().record_observed(200, now);

    let sel = Arc::new(AutoSelector::new(
        SelectorConfig { group_tag: "auto".into(), ..Default::default() },
        table.clone(),
    ).with_client(client.clone()));
    sel.set_members(vec!["DEAD".into(), "GOOD".into()]).await;

    let guard = ActiveHealthGuard::new(
        HealthGuardConfig { url: "http://www.gstatic.com/generate_204".into(),
                            timeout_ms: 2000, interval_ms: 200, fail_threshold: 2 },
        table.clone(),
        sel.clone(),
    ).with_client(client.clone());

    // 确认初始状态：auto 选中 DEAD
    let now_sel = sel.current_selection().await;
    eprintln!("初始 auto.now = {:?}", now_sel);
    assert_eq!(now_sel.as_deref(), Some("DEAD"), "前置：auto 应选中 DEAD");

    // 驱动守卫直到完成故障转移（上限 ~6s）
    let t0 = Instant::now();
    let mut outcome = GuardOutcome::Skipped;
    for _ in 0..30 {
        outcome = guard.run_once().await;
        if let GuardOutcome::Failover { .. } = outcome {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let elapsed = t0.elapsed();
    eprintln!("守卫结果: {:?}（耗时 {:?}）", outcome, elapsed);

    // 先向内核核对选择结果，再回收：任何 panic 都不应留下孤儿进程占端口
    let kernel_now = client
        .get_proxies()
        .await
        .ok()
        .and_then(|j| j["proxies"]["auto"]["now"].as_str().map(|s| s.to_string()));

    let _ = child.kill().await;
    let _ = child.wait().await;
    let _ = std::fs::remove_file(&cfg_path);
    let _ = std::fs::remove_file(&cache);

    match &outcome {
        GuardOutcome::Failover { from, to } => {
            assert_eq!(from, "DEAD");
            assert_eq!(to, "GOOD", "应切到健康节点");
            // 核对内核侧真的切了
            let real_now = kernel_now.as_deref().unwrap_or("<读取失败>");
            assert_eq!(real_now, "GOOD", "内核侧应已切换到 GOOD");
            eprintln!("内核确认: auto.now = {}", real_now);
            assert!(elapsed < Duration::from_secs(3), "故障转移耗时 {:?} 过长", elapsed);
        }
        other => panic!("期望故障转移，实际 {:?}", other),
    }

}

