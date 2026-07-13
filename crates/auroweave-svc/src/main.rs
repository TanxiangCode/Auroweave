/// Windows 系统服务宿主入口
/// 作者: TanXiang
mod installer;
mod service;
mod core_manager;
mod ipc;

use tracing::error;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(|s| s.as_str()).unwrap_or("run");

    match subcommand {
        "install" => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .init();
            if let Err(e) = crate::installer::install() {
                eprintln!("服务安装失败: {}", e);
                std::process::exit(1);
            }
            println!("服务安装成功！");
        }
        "uninstall" => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
                .init();
            if let Err(e) = crate::installer::uninstall() {
                eprintln!("服务卸载失败: {}", e);
                std::process::exit(1);
            }
            println!("服务卸载成功！");
        }
        "run" => {
            init_file_logging();
            if let Err(e) = crate::service::run() {
                error!("运行服务主体失败: {}", e);
                std::process::exit(1);
            }
        }
        _ => {
            println!("未知子命令。用法:");
            println!("  auroweave-svc install     - 安装 Windows 服务并锁定 Token");
            println!("  auroweave-svc uninstall   - 停止并卸载 Windows 服务");
            println!("  auroweave-svc run         - 以服务主体方式由 SCM 拉起运行 (默认)");
        }
    }
}

fn init_file_logging() {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let log_dir = std::path::PathBuf::from(program_data).join("Auroweave");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("service.log");

    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)
    {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
            .with_writer(file)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
    }
}
