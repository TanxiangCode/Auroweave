#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
/// Windows 系统服务宿主入口
/// 作者: TanXiang
#[cfg(target_os = "windows")]
mod installer;
#[cfg(target_os = "windows")]
mod service;
mod core_manager;
#[cfg(target_os = "windows")]
mod ipc;
mod updater;
mod utils;

use tracing::error;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(|s| s.as_str()).unwrap_or("run");

    // 简单解析可选的 --singbox 或 -s 参数
    let mut singbox_path = None;
    for i in 0..args.len() {
        if (args[i] == "--singbox" || args[i] == "-s") && i + 1 < args.len() {
            singbox_path = Some(args[i + 1].as_str());
        }
    }

    match subcommand {
        "install" | "install-service" => {
            #[cfg(target_os = "windows")]
            {
                tracing_subscriber::fmt()
                    .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                    .init();
                if let Err(e) = crate::installer::install_service(singbox_path) {
                    eprintln!("系统服务模式安装失败: {}", e);
                    std::process::exit(1);
                }
                println!("系统服务模式安装成功！");
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = singbox_path;
                eprintln!("install-service 命令仅支持 Windows 平台");
                std::process::exit(1);
            }
        }
        "install-task" => {
            #[cfg(target_os = "windows")]
            {
                tracing_subscriber::fmt()
                    .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                    .init();
                if let Err(e) = crate::installer::install_task(singbox_path) {
                    eprintln!("计划任务模式安装失败: {}", e);
                    std::process::exit(1);
                }
                println!("计划任务模式安装成功！");
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = singbox_path;
                eprintln!("install-task 命令仅支持 Windows 平台");
                std::process::exit(1);
            }
        }
        "uninstall" => {
            #[cfg(target_os = "windows")]
            {
                tracing_subscriber::fmt()
                    .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                    .init();
                if let Err(e) = crate::installer::uninstall() {
                    eprintln!("组件卸载失败: {}", e);
                    std::process::exit(1);
                }
                println!("组件卸载成功！");
            }
            #[cfg(not(target_os = "windows"))]
            {
                eprintln!("uninstall 命令仅支持 Windows 平台");
                std::process::exit(1);
            }
        }
        "run" => {
            #[cfg(target_os = "windows")]
            {
                init_file_logging();
                if let Err(e) = crate::service::run() {
                    error!("运行服务主体失败: {}", e);
                    std::process::exit(1);
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                eprintln!("run 命令仅支持 Windows 平台");
                std::process::exit(1);
            }
        }
        "run-task" => {
            init_file_logging();
            if let Err(e) = run_direct_task() {
                error!("运行计划任务模式失败: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            println!("未知子命令。用法:");
            println!("  AuroDaemon install-service --singbox <path> - 安装为 Windows 服务并锁定 Token");
            println!("  AuroDaemon install-task    --singbox <path> - 安装为提权计划任务（本地运行模式使用）");
            println!("  AuroDaemon uninstall                        - 停止并卸载所有 Windows 服务/计划任务");
            println!("  AuroDaemon run                              - 以服务主体方式由 SCM 拉起运行 (默认)");
            println!("  AuroDaemon run-task                         - 由计划任务提权启动的 sing-box 托管模式");
        }
    }
}

/// 初始化按天轮转的文件日志
///
/// 轮转策略：
/// - 文件名带日期后缀：`service_yyyyMMdd.log`（日期由 Unix epoch 天数换算得出，
///   采用 UTC，svc crate 不引入 chrono 依赖）
/// - 每天一个新文件，跨天重启/长时间运行时自然切换到新文件
/// - 启动时清理超过 7 天的旧日志文件，防止日志无限增长
///
/// 说明：服务进程通常随计划任务/服务启停，每次启动都会走到本函数，
/// 因此"跨天写入新文件"的实际效果由启动时的日期决定；单个文件最长
/// 覆盖一个自然天的日志量，配合 7 天清理上限，磁盘占用可控。
fn init_file_logging() {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let log_dir = std::path::PathBuf::from(program_data).join("Auroweave");
    let logs_dir = log_dir.join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);

    // 清理 7 天前的旧日志
    cleanup_old_logs(&logs_dir);

    let day = unix_day_now();
    let log_path = logs_dir.join(format!("service_{}.log", format_day(day)));

    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(log_path)
    {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
            .with_writer(file)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    }
}

/// 计算当前 UTC 日期对应的 Unix epoch 天数（无外部依赖的日期换算）
fn unix_day_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86400) as i64)
        .unwrap_or(0)
}

/// 将 Unix epoch 天数格式化为 `yyyyMMdd`（基于 civil-from-days 算法，
/// 仅用整数运算，不引入 chrono）
fn format_day(days: i64) -> String {
    // Howard Hinnant 的 civil_from_days 算法：
    // 将 epoch 起的天数转换为公历年月日
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}{:02}{:02}", y, m, d)
}

/// 清理超过保留期限（7 天）的旧日志文件
///
/// 仅删除匹配 `service_yyyyMMdd.log` 命名模式的文件，
/// 解析文件名中的日期与当前日期比较，相差超过 7 天即删除。
fn cleanup_old_logs(logs_dir: &std::path::Path) {
    const RETAIN_DAYS: i64 = 7;
    let today = unix_day_now();
    let Ok(entries) = std::fs::read_dir(logs_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // 形如 service_20260903.log
        if let Some(stem) = name.strip_prefix("service_").and_then(|s| s.strip_suffix(".log")) {
            if stem.len() != 8 || !stem.chars().all(|c| c.is_ascii_digit()) {
                continue; // 非日期命名的文件不动
            }
            let (y, rest) = stem.split_at(4);
            let (m, d) = rest.split_at(2);
            let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i64>(), m.parse::<i64>(), d.parse::<i64>()) else {
                continue;
            };
            // days_from_civil（civil_from_days 的逆运算）
            let yy = if m <= 2 { y - 1 } else { y };
            let era = yy.div_euclid(400);
            let yoe = yy.rem_euclid(400);
            let mp = if m > 2 { m - 3 } else { m + 9 };
            let doy = (153 * mp + 2) / 5 + d - 1;
            let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
            let file_day = era * 146097 + doe - 719468;
            if today - file_day > RETAIN_DAYS {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

fn run_direct_task() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("以计划任务提权模式启动 sing-box 托管...");

    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let cache_dir = std::path::PathBuf::from(program_data).join("Auroweave");
    let config_path = cache_dir.join("config").join("config.json");
    if !config_path.exists() {
        return Err("找不到缓存的 config.json 配置文件，请确认主程序是否已写入".into());
    }

    let config_content = std::fs::read_to_string(&config_path)?;

    // 初始化 Tokio 运行时（失败时写日志后退出，而非 unwrap panic——
    // panic 信息在 windows_subsystem=windows 的静默进程中不可见，日志是唯一留痕）
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            error!("初始化 Tokio 运行时失败: {}", e);
            return Err(format!("初始化 Tokio 运行时失败: {}", e).into());
        }
    };

    rt.block_on(async {
        // 先检查更新
        updater::check_and_apply_updates();

        let core_manager = crate::core_manager::CoreManager::new();
        core_manager.start(&config_content).await.map_err(|e| format!("启动 sing-box 失败: {}", e))?;
        tracing::info!("sing-box 进程启动成功，任务模式开始挂起。");
        
        // 读取 parent.pid 文件并监控
        let pid_file = cache_dir.join("config").join("parent.pid");
        let parent_pid = std::fs::read_to_string(&pid_file)
            .unwrap_or_default()
            .trim()
            .parse::<u32>()
            .unwrap_or(0);

        if parent_pid != 0 {
            tracing::info!("已锁定父进程 PID: {}，看门狗已启动...", parent_pid);
        } else {
            tracing::warn!("无法读取父进程 PID 文件，看门狗未能正确初始化。");
        }

        // PID 复用防护：记录看门狗启动时父进程的创建时间（FILETIME）。
        // 后续每次轮询若发现同 PID 进程的创建时间与初值不符，说明原父进程
        // 已退出、PID 被系统复用到无关进程，看门狗必须立即退出，
        // 否则会无限期挂起（等待一个永不退出的陌生进程）。
        let parent_creation_time: Option<u64> = if parent_pid != 0 {
            #[cfg(target_os = "windows")]
            {
                get_process_creation_time(parent_pid)
            }
            #[cfg(not(target_os = "windows"))]
            {
                None
            }
        } else {
            None
        };
        if parent_pid != 0 && parent_creation_time.is_none() {
            tracing::warn!("无法获取父进程创建时间，PID 复用校验退化为仅存活检测。");
        }

        // 挂起进程，并周期性检测父进程是否存活
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if parent_pid != 0 {
                #[cfg(target_os = "windows")]
                {
                    // 在 Windows 下通过尝试打开进程句柄来判断存活
                    unsafe {
                        use windows_sys::Win32::Foundation::CloseHandle;
                        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, GetExitCodeProcess};

                        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, parent_pid);
                        if handle == 0 {
                            tracing::info!("父进程 {} 已不存在，看门狗触发退出。", parent_pid);
                            break;
                        } else {
                            // PID 复用校验：同 PID 进程的创建时间与初值不符，
                            // 说明原父进程已退出、PID 已被复用，立即退出看门狗
                            if let Some(initial) = parent_creation_time {
                                if get_process_creation_time(parent_pid) != Some(initial) {
                                    CloseHandle(handle);
                                    tracing::info!("父进程 PID {} 已被其他进程复用（创建时间变化），看门狗触发退出。", parent_pid);
                                    break;
                                }
                            }
                            let mut exit_code: u32 = 0;
                            if GetExitCodeProcess(handle, &mut exit_code) != 0 && exit_code != 259 { // 259 = STILL_ACTIVE
                                tracing::info!("父进程 {} 已退出 (状态码 {})，看门狗触发退出。", parent_pid, exit_code);
                                CloseHandle(handle);
                                break;
                            }
                            CloseHandle(handle);
                        }
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    // Unix 下通过 kill -0 检测进程是否存在（无创建时间校验原语，
                    // PID 复用风险已知但本平台仅用于开发调试，生产看门狗仅 Windows）
                    let result = std::process::Command::new("kill")
                        .arg("-0")
                        .arg(parent_pid.to_string())
                        .status();
                    if !result.map(|s| s.success()).unwrap_or(false) {
                        tracing::info!("父进程 {} 已不存在，看门狗触发退出。", parent_pid);
                        break;
                    }
                }
            }
        }
        
        // 优雅停止 core_manager
        let _ = core_manager.stop().await;

        Ok::<(), String>(())
    })?;

    Ok(())
}

/// 查询指定 PID 进程的创建时间（Windows FILETIME，100ns 刻度，u64）
///
/// 供看门狗做 PID 复用检测：同一 PID 在不同时期对应不同进程时，
/// 创建时间必然不同。进程已退出或查询失败返回 None。
#[cfg(target_os = "windows")]
fn get_process_creation_time(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, GetProcessTimes, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return None;
        }
        let mut creation: FILETIME = std::mem::zeroed();
        let mut exit_time: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();
        let ok = GetProcessTimes(
            handle,
            &mut creation,
            &mut exit_time,
            &mut kernel,
            &mut user,
        );
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        Some(((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64)
    }
}
