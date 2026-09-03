#![cfg(target_os = "windows")]
/// Windows 服务主体接入实现
/// 作者: TanXiang
use std::ffi::OsString;
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::{error, info};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
};
use crate::core_manager::CoreManager;
use crate::ipc::IpcServer;

const SERVICE_NAME: &str = "AuroweaveCoreService";

define_windows_service!(ffi_service_main, service_main);

pub fn run() -> Result<(), windows_service::Error> {
    windows_service::service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        error!("服务异常退出: {:?}", e);
    }
}

fn run_service() -> Result<(), Box<dyn std::error::Error>> {
    info!("服务进程初始化...");

    // 创建退出信号管道
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let shutdown_tx = Arc::new(std::sync::Mutex::new(Some(shutdown_tx)));

    // 注册服务控制回调句柄
    let tx_clone = shutdown_tx.clone();
    let status_handle = service_control_handler::register(SERVICE_NAME, move |control_event| {
        match control_event {
            ServiceControl::Stop => {
                info!("收到系统服务 Stop 请求...");
                // 触发退出信号
                if let Ok(mut guard) = tx_clone.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(());
                    }
                }
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Shutdown => {
                // Shutdown 事件（系统关机）当前与 Stop 走相同的退出路径，
                // 至少记录日志留痕，便于排查"服务为何退出"
                info!("收到系统 Shutdown 事件（系统正在关机），执行与 Stop 相同的退出路径...");
                if let Ok(mut guard) = tx_clone.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(());
                    }
                }
                ServiceControlHandlerResult::NoError
            }
            other => {
                // 其余控制事件（含低级别硬件事件）不处理，记录日志后忽略
                info!("收到未处理的系统服务控制事件: {:?}", other);
                ServiceControlHandlerResult::NoError
            }
        }
    })?;

    // 上报启动中状态
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(2),
        process_id: None,
    })?;

    // 初始化 Tokio 运行时
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let core_manager = Arc::new(CoreManager::new());
    let ipc_server = Arc::new(IpcServer::new(core_manager.clone()));

    // 上报运行中状态
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    info!("系统服务状态已切换为 Running，正在运行 IPC 服务循环。");

    // 运行 IPC 服务并阻塞直到收到退出信号
    rt.block_on(async {
        ipc_server.run(shutdown_rx).await;
    });

    info!("IPC 服务已关闭，开始执行资源清理程序...");

    // 上报停止中状态
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::from_secs(5),
        process_id: None,
    })?;

    // 优雅停止 sing-box 进程
    rt.block_on(async {
        if let Err(e) = core_manager.stop().await {
            error!("停止托管代理进程时出错: {}", e);
        }
    });

    // 上报已停止状态
    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    info!("系统服务已彻底退出，工作结束。");
    Ok(())
}
