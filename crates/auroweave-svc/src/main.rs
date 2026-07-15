#![windows_subsystem = "windows"]
/// Windows 系统服务宿主入口
/// 作者: TanXiang
mod installer;
mod service;
mod core_manager;
mod ipc;
mod updater;

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
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .init();
            if let Err(e) = crate::installer::install_service(singbox_path) {
                eprintln!("系统服务模式安装失败: {}", e);
                std::process::exit(1);
            }
            println!("系统服务模式安装成功！");
        }
        "install-task" => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .init();
            if let Err(e) = crate::installer::install_task(singbox_path) {
                eprintln!("计划任务模式安装失败: {}", e);
                std::process::exit(1);
            }
            println!("计划任务模式安装成功！");
        }
        "uninstall" => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .init();
            if let Err(e) = crate::installer::uninstall() {
                eprintln!("组件卸载失败: {}", e);
                std::process::exit(1);
            }
            println!("组件卸载成功！");
        }
        "run" => {
            init_file_logging();
            if let Err(e) = crate::service::run() {
                error!("运行服务主体失败: {}", e);
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

fn init_file_logging() {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let log_dir = std::path::PathBuf::from(program_data).join("Auroweave");
    let logs_dir = log_dir.join("logs");
    let _ = std::fs::create_dir_all(&logs_dir);
    let log_path = logs_dir.join("service.log");

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

fn run_direct_task() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("以计划任务提权模式启动 sing-box 托管...");

    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let cache_dir = std::path::PathBuf::from(program_data).join("Auroweave");
    let config_path = cache_dir.join("config").join("config.json");
    if !config_path.exists() {
        return Err("找不到缓存的 config.json 配置文件，请确认主程序是否已写入".into());
    }

    let config_content = std::fs::read_to_string(&config_path)?;

    // 初始化 Tokio 运行时
    let rt = tokio::runtime::Runtime::new().unwrap();

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

        // 挂起进程，并周期性检测父进程是否存活
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if parent_pid != 0 {
                // 在 Windows 下通过尝试打开进程句柄来判断存活
                unsafe {
                    use windows_sys::Win32::Foundation::CloseHandle;
                    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, GetExitCodeProcess};
                    
                    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, parent_pid);
                    if handle == 0 {
                        tracing::info!("父进程 {} 已不存在，看门狗触发退出。", parent_pid);
                        break;
                    } else {
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
        }
        
        // 优雅停止 core_manager
        let _ = core_manager.stop().await;
        
        Ok::<(), String>(())
    })?;

    Ok(())
}
