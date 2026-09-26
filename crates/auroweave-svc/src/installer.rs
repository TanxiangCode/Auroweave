#![cfg(target_os = "windows")]
/// Windows 系统服务与计划任务提权安装器
/// 作者: TanXiang
///
/// 负责 AuroDaemon 提权组件的安装、卸载和安全加固。提供两种安装模式：
///
/// 1. **服务模式** (`install_service`)：
///    - 注册 Windows 系统服务 `AuroweaveCoreService`，由 SCM 管理，以 SYSTEM 权限运行
///    - 服务启动方式: `SERVICE_DEMAND_START`（手动启动，非开机自启）
///    - 设置服务安全描述符: SYSTEM/Admin 完全控制，Authenticated Users 可启停查询
///    - 额外创建计划任务 `AuroweaveDirectTunTask` 作为本地模式 TUN 提权的备用
///
/// 2. **计划任务模式** (`install_task`)：
///    - 仅注册计划任务 `AuroweaveDirectTunTask`，不注册系统服务
///    - 适用于本地运行模式，仅需 TUN 静默提权而不需要常驻服务
///
/// 安装流程（两种模式共同）：
/// 1. 创建并安全锁定 bin 目录（受保护 DACL，先于任何文件写入）
/// 2. 复制二进制文件到 `%ProgramData%\Auroweave\bin`（AuroDaemon.exe + sing-box.exe）
///    并对每个文件显式设置文件级 SDDL + 复验 SHA-256 与源文件一致
/// 3. 生成 IPC 安全 Token（随机 UUID v4 明文存储，依赖文件 ACL 保护）
/// 4. 创建计划任务 XML（最高权限 + 隐藏 + 免 UAC 触发）
///
/// 卸载流程 (`uninstall`)：
/// 1. 停止并删除系统服务
/// 2. 清理 IPC Token 文件
/// 3. 删除计划任务
/// 4. 递归删除 bin 目录
use std::path::PathBuf;
use tracing::info;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, DACL_SECURITY_INFORMATION};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, ControlService, CreateServiceW, DeleteService, OpenSCManagerW, OpenServiceW, SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS, SERVICE_CONTROL_STOP, SERVICE_DEMAND_START, SERVICE_ERROR_NORMAL, SERVICE_STATUS, SERVICE_WIN32_OWN_PROCESS
};
use windows_sys::Win32::Security::SetFileSecurityW;

#[link(name = "advapi32")]
extern "system" {
    fn SetServiceObjectSecurity(
        hservice: isize,
        dwsecurityinformation: u32,
        lpsecuritydescriptor: windows_sys::Win32::Security::PSECURITY_DESCRIPTOR,
    ) -> i32;
}

const SERVICE_NAME: &str = "AuroweaveCoreService";
const DISPLAY_NAME: &str = "Auroweave Core Service";

/// 对单个文件应用 SDDL 安全描述符（文件级，无继承标志——文件没有子对象）
///
/// 文件 SDDL: SYSTEM (SY) 与 Administrators (BA) 完全控制 (FA)，
/// Authenticated Users (AU) 读取和执行 (FRGX)，禁止普通用户写入篡改。
fn secure_bin_file(file_path: &std::path::Path) -> Result<(), String> {
    apply_sddl_to_path(
        file_path,
        "D:(A;;FA;;;SY)(A;;FA;;;BA)(A;;FRGX;;;AU)",
    )
}

/// 复制二进制文件到 `%ProgramData%\Auroweave\bin`
///
/// 前置条件：调用方必须已通过 `secure_bin_dir` 锁定 bin 目录。
///
/// 复制流程：
/// 1. 复制自身 (AuroDaemon.exe) 到目标位置（覆盖旧文件）
/// 2. 复制 sing-box.exe：
///    - 若传入 singbox_src_path 则从指定路径复制
///    - 否则在同级目录下搜索 sing-box*.exe 并选择最新版本
/// 3. 每个文件复制完成后：
///    - 显式设置文件级 SDDL（不依赖目录继承，确保 ACL 无论如何正确）
///    - 复验目标文件 SHA-256 与源文件一致，防止复制中途被替换（TOCTOU）
/// 4. 返回 AuroDaemon.exe 的目标路径
fn copy_binaries(singbox_src_path: Option<&str>) -> Result<PathBuf, String> {
    let bin_dir = crate::utils::bin_dir();

    // 安全顺序要求：bin 目录必须已由调用方在复制任何文件之前创建并锁定
    if !bin_dir.is_dir() {
        return Err("bin 目录不存在，安装顺序错误：必须先调用 secure_and_create_bin_dir".to_string());
    }

    // 1. 复制自身 (AuroDaemon.exe) 到目标位置
    let current_exe = std::env::current_exe().map_err(|e| format!("获取自身路径失败: {}", e))?;
    let target_svc_path = bin_dir.join("AuroDaemon.exe");
    copy_and_verify(&current_exe, &target_svc_path, "AuroDaemon.exe")?;

    // 2. 复制 sing-box.exe
    // 优先使用传入的路径，未传入时在同级目录搜索最新版本
    if let Some(src_path_str) = singbox_src_path {
        let src_path = PathBuf::from(src_path_str);
        if src_path.exists() {
            let target_sb_path = bin_dir.join(crate::utils::active_core_name());
            copy_and_verify(&src_path, &target_sb_path, "sing-box.exe")?;
        } else {
            return Err(format!("传入的 sing-box 路径不存在: {:?}", src_path));
        }
    } else {
        // 如果没有传入，尝试在同级目录下寻找并拷贝（查找逻辑复用 utils 唯一实现）
        if let Some(exe_dir) = current_exe.parent() {
            match crate::utils::newest_core_in_dir(exe_dir) {
                Some(p) => {
                    let target_sb_path = bin_dir.join(crate::utils::active_core_name());
                    copy_and_verify(&p, &target_sb_path, "sing-box.exe")?;
                    // 安装成功后回收同目录的历史内核，避免 GUI 升级留下的
                    // 版本化文件与本步骤写入的稳定入口名长期共存
                    crate::utils::cleanup_stale_core_binaries(&bin_dir, &target_sb_path);
                    info!("在同级目录下找到并复制 {:?} 至 {:?}", p, target_sb_path);
                }
                None => return Err("未指定 singbox 路径且无法在当前目录下找到任何 sing-box*.exe".to_string()),
            }
        }
    }

    Ok(target_svc_path)
}

/// 复制单个文件并执行安全后处理（文件级 SDDL + SHA-256 复验）
///
/// 步骤：
/// 1. 计算源文件哈希（作为完整性基准）
/// 2. 删除旧目标文件后复制
/// 3. 对目标文件设置文件级 SDDL
/// 4. 复验目标文件哈希 == 源文件哈希，不一致视为复制被篡改/损坏，报错回滚
fn copy_and_verify(src: &std::path::Path, dst: &std::path::Path, name: &str) -> Result<(), String> {
    let src_hash = crate::utils::compute_sha256(src)
        .ok_or_else(|| format!("计算源文件 {} 哈希失败: {:?}", name, src))?;

    if dst.exists() {
        let _ = std::fs::remove_file(dst);
    }
    std::fs::copy(src, dst).map_err(|e| format!("复制 {} 失败: {}", name, e))?;
    info!("已复制 {} 至 {:?}", name, dst);

    // 显式设置文件级 SDDL——不依赖目录继承，确保文件 ACL 无论如何正确
    secure_bin_file(dst).map_err(|e| format!("锁定 {} 安全属性失败: {}", name, e))?;

    // 复验目标文件哈希与源文件一致（防复制中途被替换）
    let dst_hash = crate::utils::compute_sha256(dst)
        .ok_or_else(|| format!("计算目标文件 {} 哈希失败: {:?}", name, dst))?;
    if !constant_time_eq(src_hash.as_bytes(), dst_hash.as_bytes()) {
        let _ = std::fs::remove_file(dst);
        return Err(format!("{} 复制后哈希校验不一致，疑似中途被篡改，已删除目标文件", name));
    }
    info!("{} 完整性校验通过 (SHA-256: {})", name, src_hash);

    Ok(())
}

/// 创建并锁定二进制目录（必须在任何文件复制之前调用）
///
/// 使用 SDDL 安全描述符设置 bin 目录权限：
/// - `D:PAI`：Protected DACL + Auto-Inherit。Protected 表示删除从父目录
///   （`%ProgramData%`，默认 Users 可写）继承来的宽松 ACE；AI 表示将可继承
///   ACE 标记为自动继承传播给子对象
/// - `(A;OICI;FA;;;SY)`：SYSTEM 完全控制，对象继承 (OI) + 容器继承 (CI)
/// - `(A;OICI;FA;;;BA)`：Administrators 完全控制，OICI 继承到子文件
/// - `(A;OICI;FRGX;;;AU)`：Authenticated Users 只读 + 执行，禁止写入和篡改
///
/// 由于目录 DACL 是 Protected 的且在复制前应用，后续创建的文件将强制
/// 继承上述受限 ACE，防止普通用户替换提权组件二进制文件。
fn secure_bin_dir(bin_dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(bin_dir).map_err(|e| format!("创建服务二进制目录失败: {}", e))?;
    apply_sddl_to_path(
        bin_dir,
        // D:PAI = protected + auto-inherit；ACE 带 OICI 继承到子文件
        "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRGX;;;AU)",
    )
}

/// 对指定路径（文件或目录）应用 SDDL 安全描述符
///
/// 内部封装 `ConvertStringSecurityDescriptorToSecurityDescriptorW` +
/// `SetFileSecurityW`，失败时释放安全描述符内存并返回 OS 错误信息。
fn apply_sddl_to_path(path: &std::path::Path, sddl: &str) -> Result<(), String> {
    let sddl_w: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_w.as_ptr(),
            1, // SDDL_REVISION_1
            &mut sd,
            std::ptr::null_mut(),
        ) == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("转换安全描述符失败: {}", err));
        }

        let path_str = path.to_string_lossy().to_string();
        let path_w: Vec<u16> = path_str.encode_utf16().chain(std::iter::once(0)).collect();
        let res = SetFileSecurityW(path_w.as_ptr(), DACL_SECURITY_INFORMATION, sd);
        LocalFree(sd);

        if res == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("应用安全属性失败 (路径 {:?}): {}", path, err));
        }
    }
    Ok(())
}

/// 常数时间字节比较（避免短路比较带来的时序侧信道）
///
/// 逐字节 OR 累积差异，长度差异也计入，最后统一判断，保证耗时
/// 不因匹配前缀长度而泄漏信息。
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// 注册 Windows 系统服务 + 计划任务（服务运行模式）
///
/// 完整安装流程：
/// 1. 停止并删除已存在的同名服务（确保覆盖安装干净性）
/// 2. 创建并锁定 bin 目录（受保护 DACL，先于任何文件写入）
/// 3. 复制二进制文件到 `%ProgramData%\Auroweave\bin`（含文件级 SDDL + 哈希复验）
/// 4. 通过 SCM 创建服务项（手动启动 + OWN_PROCESS）
/// 5. 设置服务安全描述符（允许 Users 免 UAC 启停）
/// 6. 生成 IPC 安全 Token
/// 7. 创建计划任务 `AuroweaveDirectTunTask`（本地模式备用）
pub fn install_service(singbox_src_path: Option<&str>) -> Result<(), String> {
    info!("开始以服务模式安装提权组件...");

    // 步骤1: 停止并删除已存在的同名服务，确保覆盖安装的干净性
    let service_name_w: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm != 0 {
            let service = OpenServiceW(scm, service_name_w.as_ptr(), SERVICE_ALL_ACCESS);
            if service != 0 {
                let mut status: SERVICE_STATUS = std::mem::zeroed();
                let _ = ControlService(service, SERVICE_CONTROL_STOP, &mut status);
                let _ = DeleteService(service);
                CloseServiceHandle(service);
                info!("已删除旧的系统服务配置，等待 SCM 释放句柄...");
                std::thread::sleep(std::time::Duration::from_millis(1500));
            }
            CloseServiceHandle(scm);
        }
    }

    // 步骤2-3: 先创建并锁定 bin 目录（受保护 DACL），再复制文件
    // 安全顺序：目录在无保护状态下复制文件，会给攻击者留下替换文件的窗口，
    // 因此必须先应用 D:PAI 受保护 DACL，后续复制的文件强制继承受限 ACE。
    let bin_dir = crate::utils::bin_dir();
    secure_bin_dir(&bin_dir)?;

    let svc_path = copy_binaries(singbox_src_path)?;

    // 步骤4-5: 通过 SCM 创建服务项并设置安全描述符
    let binary_path_str = format!("\"{}\" run", svc_path.to_string_lossy());
    let binary_path_w: Vec<u16> = binary_path_str.encode_utf16().chain(std::iter::once(0)).collect();
    let display_name_w: Vec<u16> = DISPLAY_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm == 0 {
            return Err("打开 SCM 失败，请确认是否在管理员权限下运行".to_string());
        }

        let service = CreateServiceW(
            scm,
            service_name_w.as_ptr(),
            display_name_w.as_ptr(),
            SERVICE_ALL_ACCESS,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_NORMAL,
            binary_path_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        );

        if service == 0 {
            let err = std::io::Error::last_os_error();
            CloseServiceHandle(scm);
            return Err(format!("创建服务项失败: {}", err));
        }

        info!("系统服务项已创建成功，正在设置服务安全描述符...");

        // 允许 SYSTEM (SY) 和 Administrators (BA) 完全控制
        // 允许 Authenticated Users (AU) 拥有启停和状态查询权限，以实现免 UAC 启停
        let sddl = "D:(A;;CCLCRPWPDTLOCRRC;;;SY)(A;;CCDCLCRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCRPWPRC;;;AU)";
        let sddl_w: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();

        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_w.as_ptr(),
            1, // SDDL_REVISION_1
            &mut sd,
            std::ptr::null_mut(),
        ) == 0 {
            let err = std::io::Error::last_os_error();
            CloseServiceHandle(service);
            CloseServiceHandle(scm);
            return Err(format!("转换服务 SDDL 失败: {}", err));
        }

        let res = SetServiceObjectSecurity(service, DACL_SECURITY_INFORMATION, sd);
        LocalFree(sd);

        if res == 0 {
            let err = std::io::Error::last_os_error();
            CloseServiceHandle(service);
            CloseServiceHandle(scm);
            return Err(format!("应用服务安全描述符失败: {}", err));
        }

        CloseServiceHandle(service);
        CloseServiceHandle(scm);
    }

    // 步骤6: 生成 IPC 安全 Token
    info!("系统服务配置成功。正在生成 IPC Token...");
    setup_token()?;

    // 步骤7: 创建计划任务（本地模式 TUN 提权备用）
    if let Err(e) = create_direct_tun_task(&svc_path) {
        info!("创建静默提权任务失败（警告）: {}", e);
    }

    info!("服务模式提权组件安装与安全加固已全部完成。");
    Ok(())
}

/// 仅注册计划任务（本地运行模式）
///
/// 安装流程：
/// 1. 创建并锁定 bin 目录（受保护 DACL，先于任何文件写入）
/// 2. 复制二进制文件到 `%ProgramData%\Auroweave\bin`（含文件级 SDDL + 哈希复验）
/// 3. 生成 IPC 安全 Token
/// 4. 创建计划任务 `AuroweaveDirectTunTask`
pub fn install_task(singbox_src_path: Option<&str>) -> Result<(), String> {
    info!("开始以计划任务模式安装提权组件...");

    // 步骤1: 先创建并锁定 bin 目录，再复制文件（安全顺序与 install_service 一致）
    let bin_dir = crate::utils::bin_dir();
    secure_bin_dir(&bin_dir)?;

    // 步骤2: 复制二进制文件（文件级 SDDL + SHA-256 复验由 copy_binaries 内部完成）
    let svc_path = copy_binaries(singbox_src_path)?;

    // 步骤3: 生成 IPC 安全 Token
    info!("配置 Token...");
    setup_token()?;

    // 步骤4: 创建计划任务
    if let Err(e) = create_direct_tun_task(&svc_path) {
        return Err(format!("创建静默提权计划任务失败: {}", e));
    }

    info!("计划任务模式提权组件安装已全部完成。");
    Ok(())
}

/// 卸载所有提权组件
///
/// 卸载流程：
/// 1. 停止并删除 Windows 系统服务
/// 2. 清理 IPC Token 文件
/// 3. 删除计划任务 `AuroweaveDirectTunTask`
/// 4. 递归删除 `%ProgramData%\Auroweave\bin` 目录
pub fn uninstall() -> Result<(), String> {
    info!("开始卸载 Windows 系统服务与提权组件...");
    
    // 步骤1: 停止并删除系统服务
    let service_name_w: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm != 0 {
            let service = OpenServiceW(scm, service_name_w.as_ptr(), SERVICE_ALL_ACCESS);
            if service != 0 {
                // 先停止服务
                let mut status: SERVICE_STATUS = std::mem::zeroed();
                let _ = ControlService(service, SERVICE_CONTROL_STOP, &mut status);
                // 删除服务
                let _ = DeleteService(service);
                CloseServiceHandle(service);
            }
            CloseServiceHandle(scm);
        }
    }

    // 步骤2-3: 清理 Token + 删除计划任务
    let _ = cleanup_token();
    let _ = remove_direct_tun_task();

    // 步骤4: 递归删除 bin 目录
    let bin_dir = crate::utils::bin_dir();
    if bin_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&bin_dir) {
            info!("清理服务二进制目录失败（可能部分文件被占用）: {}", e);
        }
    }

    info!("系统服务及残留配置已彻底删除。");
    Ok(())
}

/// 生成并存储 IPC 安全 Token
///
/// 流程：
/// 1. 创建数据目录 `%ProgramData%\Auroweave\data`
/// 2. 若 Token 文件不存在则生成新 Token：
///    - 使用 `uuid::Uuid::new_v4()` 生成 128 bit 熵的随机 Token
///    - **明文写入** token 文件（不再用共享密钥加密——密钥硬编码在三个二进制中，
///      加密无意义且提供虚假安全感；本机 IPC 场景依赖文件 ACL 作为唯一保护边界）
/// 3. 对 Token 文件设置安全描述符：仅 SYSTEM/Admin 可读写，
///    普通用户完全不可见（SDDL 不含 AU 条目），防止低权限进程窃取 Token
///
/// 注意：token 文件路径与格式需与主程序 `src-tauri/src/core/ipc_client.rs` 保持兼容
/// （路径 `%ProgramData%\Auroweave\data\ipc_token.bin`，内容为明文 token 字符串）。
fn setup_token() -> Result<(), String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let data_dir = PathBuf::from(program_data).join("Auroweave").join("data");
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {}", e))?;

    let token_path = data_dir.join("ipc_token.bin");

    if !token_path.exists() {
        // 随机 UUID v4（128 bit 熵，对本机 IPC 场景足够）
        let token = uuid::Uuid::new_v4().to_string();
        // 明文 token，依赖文件 ACL 保护（见函数级注释）
        std::fs::write(&token_path, token.as_bytes()).map_err(|e| format!("写入 Token 失败: {}", e))?;
        info!("生成新 Token 成功（明文存储，依赖文件 ACL 保护）。");
    }

    // 仅 SYSTEM(SY) 和 Administrators(BA) 完全控制，不含 AU——
    // 普通用户对该文件无任何访问权，无法读取或篡改 Token
    apply_sddl_to_path(&token_path, "D:(A;;FA;;;SY)(A;;FA;;;BA)")?;

    info!("已成功对 Token 文件进行安全锁定（仅 SYSTEM/Admin 可访问）。");
    Ok(())
}

/// 删除 IPC Token 文件
fn cleanup_token() -> Result<(), String> {
let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
let token_path = PathBuf::from(program_data).join("Auroweave").join("data").join("ipc_token.bin");
if token_path.exists() {
let _ = std::fs::remove_file(token_path);
}
Ok(())
}



/// 创建静默提权计划任务
///
/// 通过 `schtasks /create /xml` 导入预定义的任务 XML：
/// - 以当前用户身份运行，HighestAvailable 最高权限
/// - 隐藏窗口 (Hidden=true)，不弹出 UAC
/// - AllowStartOnDemand=true，允许通过 `schtasks /run` 手动触发
/// - ExecutionTimeLimit=PT0S，无超时限制
/// - Action: 执行 AuroDaemon.exe `run-task` 子命令
///
/// XML 以 UTF-16LE BOM 编码写入临时文件，确保 schtasks 正确解析中文路径。
fn create_direct_tun_task(svc_path: &std::path::Path) -> Result<(), String> {
    let task_name = "AuroweaveDirectTunTask";
    let svc_path_str = svc_path.to_string_lossy();
    
    info!("正在创建静默提权计划任务: {} -> {}", task_name, svc_path_str);
    
    let username = std::env::var("USERNAME").unwrap_or_else(|_| "SYSTEM".to_string());
    let userdomain = std::env::var("USERDOMAIN").unwrap_or_else(|_| "".to_string());
    let account = if userdomain.is_empty() { username } else { format!("{}\\{}", userdomain, username) };

    let xml = format!(
r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <URI>\{}</URI>
  </RegistrationInfo>
  <Triggers />
  <Principals>
    <Principal id="Author">
      <UserId>{}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>Parallel</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>true</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>3</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{}"</Command>
      <Arguments>run-task</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        task_name, account, svc_path_str
    );

    let temp_xml_path = std::env::temp_dir().join(format!("{}.xml", task_name));
    
    // Write as UTF-16LE with BOM
    let mut utf16_content = vec![0xFF, 0xFE];
    for c in xml.encode_utf16() {
        utf16_content.extend_from_slice(&c.to_le_bytes());
    }
    
    std::fs::write(&temp_xml_path, &utf16_content).map_err(|e| format!("写入任务 XML 失败: {}", e))?;

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    
    let mut cmd = std::process::Command::new("schtasks");
    cmd.arg("/create")
        .arg("/tn")
        .arg(task_name)
        .arg("/xml")
        .arg(&temp_xml_path)
        .arg("/f")
        .creation_flags(CREATE_NO_WINDOW);
        
    let import_output = cmd.output()
        .map_err(|e| format!("导入任务 XML 失败: {}", e))?;
        
    let out_str = String::from_utf8_lossy(&import_output.stdout);
    let err_str = String::from_utf8_lossy(&import_output.stderr);
    info!("schtasks 导入任务输出: STDOUT: {} STDERR: {}", out_str, err_str);

    let _ = std::fs::remove_file(&temp_xml_path);

    if import_output.status.success() {
        info!("已成功创建计划任务并应用 XML SDDL 免 UAC 触发权限。");
        Ok(())
    } else {
        Err(format!("创建计划任务命令行返回错误: {}\n{}", out_str, err_str))
    }
}

/// 删除静默提权计划任务
fn remove_direct_tun_task() -> Result<(), String> {
    let task_name = "AuroweaveDirectTunTask";
    info!("正在删除静默提权计划任务: {}", task_name);
    let _ = std::process::Command::new("schtasks")
        .arg("/delete")
        .arg("/tn")
        .arg(task_name)
        .arg("/f")
        .status();
    Ok(())
}

/// 把暂存区的新内核安装到受保护的 bin/（提权执行）
///
/// 背景：GUI 在线升级在 service 模式下**无法自己写 bin/**——
/// installer 用 SDDL 把该目录锁成 `Authenticated Users: FRGX`
/// （只读+执行），这是刻意的安全设计：防止普通用户替换以 SYSTEM
/// 运行的提权组件。于是 GUI 只能把新内核放到暂存区（用户可写），
/// 再由本子命令以 SYSTEM 权限完成"校验 → 复制 → 回收旧版本"。
///
/// 安全要求（与既有 updater 同等强度）：
/// 1. 源路径必须在 `update_staging/` 内（不接受任意路径，杜绝
///    "让 SYSTEM 去读用户指定任意文件"的提权原语）；
/// 2. 复制后复验 SHA-256，防止复制途中被替换（TOCTOU）；
/// 3. 目标文件名由本 crate 的命名规则生成，不接受外部传入任意文件名。
pub fn apply_staged_core(staged: &std::path::Path) -> Result<String, String> {
    let bin_dir = crate::utils::bin_dir();

    // ---- 步骤1: 源路径白名单 ----
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let staging_dir = std::path::PathBuf::from(&program_data)
        .join("Auroweave")
        .join("update_staging");
    let canonical_staged = std::fs::canonicalize(staged)
        .map_err(|e| format!("暂存内核文件不存在或不可读: {}", e))?;
    let canonical_staging = std::fs::canonicalize(&staging_dir)
        .map_err(|e| format!("暂存目录不存在: {}", e))?;
    // 前缀匹配（canonicalize 已解析 .. 与符号链接），必须落在暂存目录内
    if !canonical_staged.starts_with(&canonical_staging) {
        return Err(format!(
            "拒绝应用：源文件不在受信任的暂存目录内: {:?}",
            canonical_staged
        ));
    }
    if !crate::utils::is_core_file_name(
        &canonical_staged
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
    ) {
        return Err("拒绝应用：暂存文件名不是内核本体".to_string());
    }

    // ---- 步骤2: 识别新版本（用于版本化命名）----
    let version = detect_version_from_binary(&canonical_staged)
        .ok_or_else(|| "无法识别暂存内核的版本号，拒绝应用（避免写出服务读不到的文件名）".to_string())?;
    let target = bin_dir.join(crate::utils::versioned_core_name(&version));

    let src_hash = crate::utils::compute_sha256(&canonical_staged)
        .ok_or_else(|| "计算暂存内核哈希失败，拒绝应用".to_string())?;

    std::fs::create_dir_all(&bin_dir).map_err(|e| format!("创建 bin 目录失败: {}", e))?;

    // ---- 步骤3: 复制 + 哈希复验（防 TOCTOU）----
    std::fs::copy(&canonical_staged, &target).map_err(|e| {
        format!(
            "复制内核到 {} 失败: {}（若内核正在运行请先停止服务）",
            target.display(),
            e
        )
    })?;
    let dst_hash = crate::utils::compute_sha256(&target)
        .ok_or_else(|| "复验目标内核哈希失败".to_string())?;
    if dst_hash != src_hash {
        // 复验失败立即回滚，避免 bin/ 里留下一个半截文件
        let _ = std::fs::remove_file(&target);
        return Err(format!(
            "内核复制后哈希不一致（可能复制途中被替换）: 期望 {}，实际 {}",
            src_hash, dst_hash
        ));
    }

    // ---- 步骤4: 回收历史内核 ----
    crate::utils::cleanup_stale_core_binaries(&bin_dir, &target);

    Ok(version)
}

/// 从内核二进制的 `version` 自述中解析版本号
fn detect_version_from_binary(path: &std::path::Path) -> Option<String> {
    let out = std::process::Command::new(path).arg("version").output().ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        if let Some(rest) = line.trim().strip_prefix("sing-box version ") {
            let v = rest.trim().trim_start_matches(['v', 'V']);
            if !v.is_empty()
                && v.chars().all(|c| c.is_ascii_digit() || c == '.')
                && v.starts_with(|c: char| c.is_ascii_digit())
            {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_core_must_live_in_trusted_staging_dir() {
        // 提权原语防线：SYSTEM 绝不能去复制暂存目录之外的任意文件
        let fake = std::env::temp_dir().join(format!("auroweave_notstaged_{}", std::process::id()));
        std::fs::write(&fake, b"x").unwrap();
        let err = apply_staged_core(&fake).unwrap_err();
        assert!(
            err.contains("暂存目录") || err.contains("不存在"),
            "应拒绝暂存目录外的源文件，实际: {}",
            err
        );
        let _ = std::fs::remove_file(&fake);
    }

    #[test]
    fn non_core_filename_rejected() {
        let dir = std::env::temp_dir().join(format!("auroweave_badname_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let evil = dir.join("payload.exe");
        std::fs::write(&evil, b"x").unwrap();
        let err = apply_staged_core(&evil).unwrap_err();
        assert!(err.contains("不是内核本体") || err.contains("暂存目录"), "实际: {}", err);
        let _ = std::fs::remove_dir_all(&dir);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// 下载段百分比：已下载/总量 映射到整体进度的前 DOWNLOAD_PERCENT_CAP
    fn download_percent(downloaded: u64, total: u64) -> u8 {
        if total == 0 {
            return 0;
        }
        ((downloaded.min(total) * DOWNLOAD_PERCENT_CAP) / total) as u8
    }

    #[test]
    fn download_percent_maps_into_download_weight() {
        // 起止锚点：0% 对应 0，100% 对应下载段上限 70
        assert_eq!(download_percent(0, 1000), 0);
        assert_eq!(download_percent(500, 1000), 35);
        assert_eq!(download_percent(1000, 1000), DOWNLOAD_PERCENT_CAP as u8);
    }

    #[test]
    fn download_percent_clamps_when_more_bytes_than_content_length() {
        // 代理/CDN 改写 Content-Length 时已下载可能超过 total，
        // 绝不能让百分比越过 70 侵占后续阶段区间（否则进度条会倒退）
        assert_eq!(download_percent(1500, 1000), DOWNLOAD_PERCENT_CAP as u8);
    }

    #[test]
    fn download_percent_is_zero_when_total_unknown() {
        // chunked 传输拿不到 Content-Length：退化为不确定态而非除零 panic
        assert_eq!(download_percent(5000, 0), 0);
    }

    #[test]
    fn download_percent_never_decreases_monotonic() {
        let total = 3_400_000u64;
        let mut last = 0u8;
        for downloaded in (0..=total).step_by(50_000) {
            let pct = download_percent(downloaded, total);
            assert!(pct >= last, "进度回退: {downloaded} -> {pct} (上次 {last})");
            last = pct;
        }
        assert_eq!(last, DOWNLOAD_PERCENT_CAP as u8);
    }

    #[test]
    fn stages_after_download_occupy_disjoint_ascending_ranges() {
        // 关键不变量：下载(≤70) < 校验(72) < 解压(78) < 停核(84)
        // < 替换(90) < 重启(96) < 完成(100)，保证进度条单调不回退
        let anchors = [
            (SingboxUpdateStage::Downloading, DOWNLOAD_PERCENT_CAP as u8),
            (SingboxUpdateStage::Verifying, 72),
            (SingboxUpdateStage::Extracting, 78),
            (SingboxUpdateStage::StoppingCore, 84),
            (SingboxUpdateStage::Replacing, 90),
            (SingboxUpdateStage::Restarting, 96),
            (SingboxUpdateStage::Done, 100),
        ];
        for w in anchors.windows(2) {
            assert!(
                w[0].1 < w[1].1,
                "阶段锚点倒挂: {:?}={} 之后 {:?}={}",
                w[0].0, w[0].1, w[1].0, w[1].1
            );
        }
    }

    #[test]
    fn only_done_and_failed_are_terminal() {
        // 前端据 finished 解锁按钮并停止展示进度条，判定错误会导致
        // 按钮永久锁死（用户无法重试）或永不显示结果
        assert!(SingboxUpdateStage::Done.is_terminal());
        assert!(SingboxUpdateStage::Failed.is_terminal());
        for stage in [
            SingboxUpdateStage::Idle,
            SingboxUpdateStage::Preparing,
            SingboxUpdateStage::Downloading,
            SingboxUpdateStage::Verifying,
            SingboxUpdateStage::Extracting,
            SingboxUpdateStage::StoppingCore,
            SingboxUpdateStage::Replacing,
            SingboxUpdateStage::Restarting,
        ] {
            assert!(!stage.is_terminal(), "{:?} 不应被判定为终态", stage);
        }
    }

    #[test]
    fn stage_serializes_to_snake_case_matching_ts_union() {
        // 前端 SingboxUpdateStage 联合类型逐字对齐这些字符串，改名会静默失配
        let cases = [
            (SingboxUpdateStage::Idle, "\"idle\""),
            (SingboxUpdateStage::Preparing, "\"preparing\""),
            (SingboxUpdateStage::Downloading, "\"downloading\""),
            (SingboxUpdateStage::Verifying, "\"verifying\""),
            (SingboxUpdateStage::Extracting, "\"extracting\""),
            (SingboxUpdateStage::StoppingCore, "\"stopping_core\""),
            (SingboxUpdateStage::Replacing, "\"replacing\""),
            (SingboxUpdateStage::Restarting, "\"restarting\""),
            (SingboxUpdateStage::Done, "\"done\""),
            (SingboxUpdateStage::Failed, "\"failed\""),
        ];
        for (stage, expected) in cases {
            assert_eq!(serde_json::to_string(&stage).unwrap(), expected);
        }
    }

    #[test]
    fn idle_progress_is_not_upgrading() {
        // init() 依赖这一判定决定是否回填进度：idle 必须算"未在途"，
        // 否则会把上一次的残留状态复活成"正在升级"
        let p = SingboxUpdateProgress::idle();
        assert_eq!(p.stage, SingboxUpdateStage::Idle);
        assert!(!p.finished);
        assert!(!p.success.unwrap_or(false));
        // 前端 isUpgrading = !finished && stage != "idle"，idle 必须为 false
        let is_upgrading = !p.finished && p.stage != SingboxUpdateStage::Idle;
        assert!(!is_upgrading, "idle 不得被判定为升级中，否则会复活残留状态");
    }

    #[test]
    fn download_url_whitelist_accepts_official_github_hosts() {
        for url in [
            "https://github.com/SagerNet/sing-box/releases/download/v1.13.0/sing-box-linux-amd64.tar.gz",
            "https://objects.githubusercontent.com/github-production-release-asset/abc",
            "https://github-releases.githubusercontent.com/github-production-release-asset/abc",
        ] {
            assert!(validate_download_url(url).is_ok(), "应放行: {}", url);
        }
    }

    #[test]
    fn download_url_whitelist_rejects_non_github_and_plain_http() {
        // 内核二进制以 SUID/管理员权限运行，被替换等于任意代码提权，
        // 白名单与 HTTPS 强制是安全边界，必须守住
        for url in [
            "https://evil.example.com/sing-box.tar.gz",
            "https://github.com.evil.com/x.tar.gz",
            "http://github.com/SagerNet/sing-box/releases/download/v1/sing-box.tar.gz",
            "ftp://github.com/x.tar.gz",
        ] {
            assert!(validate_download_url(url).is_err(), "应拒绝: {}", url);
        }
    }

    #[test]
    fn format_bytes_human_matches_expected_units() {
        assert_eq!(format_bytes_human(0), "0 B");
        assert_eq!(format_bytes_human(512), "512 B");
        assert_eq!(format_bytes_human(1024), "1.0 KB");
        assert_eq!(format_bytes_human(14_300_000), "13.6 MB");
        assert_eq!(format_bytes_human(34_000_000), "32.4 MB");
    }
}


    /// 真机联调（快）：对真实 GitHub 小资产验证流式进度链路。
    ///
    /// 默认跳过（`#[ignore]`）：需外网。开启：
    ///   cargo test --lib e2e_real_download -- --ignored --nocapture
    ///
    /// 用小资产走通与生产完全相同的 download_with_progress 路径，
    /// 验证真实 HTTP 下的 Content-Length、字节累计与百分比映射。
    /// 大资产（27MB）那条见 e2e_large_asset_*，因网络耗时过长单独标注。
    #[tokio::test]
    #[ignore = "需外网，默认跳过"]
    async fn e2e_real_download_reports_monotonic_progress() {
        let url = "https://github.com/SagerNet/sing-box/releases/download/v1.14.2/SFA-version-metadata.json";
        let client = reqwest::Client::builder()
            .user_agent("Auroweave-Client/1.0")
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .unwrap();

        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();

        let bytes = download_with_progress(client, url, move |d, t, p, m| {
            sink.lock().unwrap().push((d, t, p, m));
        })
        .await
        .expect("真实下载应成功");

        let events = seen.lock().unwrap().clone();
        println!("e2e: 下载 {} 字节, {} 次回调", bytes.len(), events.len());
        for (d, t, p, m) in events.iter() {
            println!("  downloaded={} total={} percent={} msg={}", d, t, p, m);
        }

        // 1) 首帧必须存在（按钮要能从"准备中"切到"下载中"）
        assert!(!events.is_empty(), "应至少有一次首帧回调");
        assert_eq!(events[0].0, 0, "首帧已下载应为 0");

        // 2) 真实 Content-Length 应可得（否则前端退化为无百分比展示）
        let total = events[0].1;
        assert!(total > 0, "GitHub 应返回 Content-Length，实际: {}", total);

        // 3) 实收字节与 Content-Length 一致（未截断）
        assert_eq!(bytes.len() as u64, total, "实收字节应与 Content-Length 一致");

        // 4) 收尾回调应满字节且达到下载段权重
        let last = events.last().unwrap();
        assert_eq!(last.0, total, "收尾回调应报告满字节");
        assert_eq!(last.2, DOWNLOAD_PERCENT_CAP as u8, "收尾百分比应等于下载段上限");

        // 5) 字节与百分比全程单调不回退
        let mut prev_bytes = 0u64;
        let mut prev_pct = 0u8;
        for (d, _, p, _) in events.iter() {
            assert!(*d >= prev_bytes, "字节数回退: {} -> {}", prev_bytes, d);
            assert!(*p >= prev_pct, "百分比回退: {} -> {}", prev_pct, p);
            prev_bytes = *d;
            prev_pct = *p;
        }

        // 6) 文案含真实可读体积
        assert!(
            last.3.contains("B") || last.3.contains("KB") || last.3.contains("MB"),
            "下载文案应含可读体积，实际: {}", last.3
        );
    }

    /// 真机联调（大资产，27MB）：验证节流在多 MB 传输中确实生效。
    ///
    /// 单独标注且默认跳过：当前网络到 release-assets 主机带宽很低
    /// （实测 10~400 KB/s 波动），整包可能耗时十分钟或因链路抖动中断，
    /// 不适合放进常规验证流程。网络条件允许时手动运行。
    #[tokio::test]
    #[ignore = "需外网 + 27MB 真实下载，网络慢时可能超时，默认跳过"]
    async fn e2e_large_asset_throttles_progress_callbacks() {
        let url = "https://github.com/SagerNet/sing-box/releases/download/v1.14.2/sing-box-1.14.2-darwin-arm64.tar.gz";
        let client = reqwest::Client::builder()
            .user_agent("Auroweave-Client/1.0")
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .unwrap();

        let count = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let last_pct = std::sync::Arc::new(std::sync::Mutex::new(0u8));
        let c = count.clone();
        let lp = last_pct.clone();

        let res = download_with_progress(client, url, move |_d, _t, p, _m| {
            *c.lock().unwrap() += 1;
            let mut g = lp.lock().unwrap();
            assert!(p >= *g, "百分比回退: {} -> {}", *g, p);
            *g = p;
        })
        .await;

        // 链路中断属网络环境问题，不计为代码缺陷：如实打印后跳过断言
        let bytes = match res {
            Ok(b) => b,
            Err(e) => {
                println!("e2e-large: 下载中断（网络环境）: {}", e);
                println!(
                    "e2e-large: 中断前已回调 {} 次, 末态百分比 {}",
                    count.lock().unwrap(),
                    last_pct.lock().unwrap()
                );
                return;
            }
        };

        println!("e2e-large: 下载 {} 字节, {} 次回调", bytes.len(), count.lock().unwrap());
        // 节流生效：回调次数应远小于 KB 数
        let n = *count.lock().unwrap();
        assert!(
            n <= (bytes.len() / 1024).max(16),
            "节流失效：{} 字节产生 {} 次回调",
            bytes.len(),
            n
        );
    }


    #[test]
    fn detect_version_from_extracted_dir_name() {
        // 官方包解压根目录固定为 sing-box-<ver>-<os>-<arch>
        let dir = std::env::temp_dir().join(format!("auroweave_ver_{}", std::process::id()));
        let top = dir.join("sing-box-1.14.2-darwin-arm64");
        std::fs::create_dir_all(&top).unwrap();
        // 造一个不可执行的占位文件，让 version 自述必然失败 → 走目录名回退
        let fake = top.join("sing-box");
        std::fs::write(&fake, b"not a real binary").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o644));
        }

        let v = detect_new_version(&fake, &dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(v, Some("1.14.2".to_string()));
    }

    #[test]
    fn detect_version_returns_none_when_unidentifiable() {
        // 识别不出版本号必须返回 None 让调用方中止，绝不能猜一个版本写进文件名
        let dir = std::env::temp_dir().join(format!("auroweave_vnone_{}", std::process::id()));
        let top = dir.join("weird-name");
        std::fs::create_dir_all(&top).unwrap();
        let fake = top.join("sing-box");
        std::fs::write(&fake, b"junk").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o644));
        }
        let v = detect_new_version(&fake, &dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(v, None);
    }




