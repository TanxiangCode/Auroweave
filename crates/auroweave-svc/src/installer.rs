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
/// 1. 复制二进制文件到 `%ProgramData%\Auroweave\bin`（AuroDaemon.exe + sing-box.exe）
/// 2. 对 bin 目录进行安全加锁（SDDL: SYSTEM/Admin 完全控制，Users 只读执行）
/// 3. 生成 IPC 安全 Token（AES-256-GCM 加密存储）
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

/// 复制二进制文件到 `%ProgramData%\Auroweave\bin`
///
/// 复制流程：
/// 1. 创建 bin 目录
/// 2. 复制自身 (AuroDaemon.exe) 到目标位置（覆盖旧文件）
/// 3. 复制 sing-box.exe：
///    - 若传入 singbox_src_path 则从指定路径复制
///    - 否则在同级目录下搜索 sing-box*.exe 并选择最新版本
/// 4. 返回 AuroDaemon.exe 的目标路径
fn copy_binaries(singbox_src_path: Option<&str>) -> Result<PathBuf, String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let bin_dir = PathBuf::from(program_data).join("Auroweave").join("bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| format!("创建服务二进制目录失败: {}", e))?;

    // 1. 复制自身 (AuroDaemon.exe) 到目标位置
    let current_exe = std::env::current_exe().map_err(|e| format!("获取自身路径失败: {}", e))?;
    let target_svc_path = bin_dir.join("AuroDaemon.exe");
    
    if target_svc_path.exists() {
        let _ = std::fs::remove_file(&target_svc_path);
    }
    std::fs::copy(&current_exe, &target_svc_path)
        .map_err(|e| format!("复制 AuroDaemon.exe 失败: {}", e))?;
    info!("已复制 AuroDaemon.exe 至 {:?}", target_svc_path);

    // 2. 复制 sing-box.exe
    // 优先使用传入的路径，未传入时在同级目录搜索最新版本
    if let Some(src_path_str) = singbox_src_path {
        let src_path = PathBuf::from(src_path_str);
        if src_path.exists() {
            let target_sb_path = bin_dir.join("sing-box.exe");
            if target_sb_path.exists() {
                let _ = std::fs::remove_file(&target_sb_path);
            }
            std::fs::copy(&src_path, &target_sb_path)
                .map_err(|e| format!("复制 sing-box.exe 失败: {}", e))?;
            info!("已复制 sing-box.exe 至 {:?}", target_sb_path);
        } else {
            return Err(format!("传入的 sing-box 路径不存在: {:?}", src_path));
        }
    } else {
        // 如果没有传入，尝试在同级目录下寻找并拷贝
        if let Some(exe_dir) = current_exe.parent() {
            let mut latest_path = None;
            let mut latest_time = std::time::SystemTime::UNIX_EPOCH;
            
            if let Ok(entries) = std::fs::read_dir(exe_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() { continue; }
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                    if file_name.starts_with("sing-box") && file_name.ends_with(".exe") {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            if let Ok(modified) = metadata.modified() {
                                if modified > latest_time {
                                    latest_time = modified;
                                    latest_path = Some(path);
                                }
                            }
                        }
                    }
                }
            }

            if let Some(p) = latest_path {
                let target_sb_path = bin_dir.join("sing-box.exe");
                if target_sb_path.exists() {
                    let _ = std::fs::remove_file(&target_sb_path);
                }
                std::fs::copy(&p, &target_sb_path)
                    .map_err(|e| format!("复制 sing-box.exe 失败: {}", e))?;
                info!("在同级目录下找到并复制 {:?} 至 {:?}", p, target_sb_path);
            } else {
                return Err("未指定 singbox 路径且无法在当前目录下找到任何 sing-box*.exe".to_string());
            }
        }
    }

    Ok(target_svc_path)
}

/// 锁定二进制目录的安全权限
///
/// 使用 SDDL 安全描述符设置 bin 目录权限：
/// - SYSTEM (SY): 完全控制 (FA)
/// - Administrators (BA): 完全控制 (FA)
/// - Authenticated Users (AU): 读取和执行 (FRGX)，禁止写入和篡改
///
/// 防止普通用户修改或替换提权组件二进制文件，避免提权漏洞。
fn secure_bin_dir(bin_dir: &std::path::Path) -> Result<(), String> {
    // SYSTEM(SY) 和 Administrators(BA) 拥有完全控制，Authenticated Users(AU) 拥有读取和执行权限
    let file_sddl = "D:(A;;FA;;;SY)(A;;FA;;;BA)(A;;FRGX;;;AU)";
    let file_sddl_w: Vec<u16> = file_sddl.encode_utf16().chain(std::iter::once(0)).collect();
    
    unsafe {
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            file_sddl_w.as_ptr(),
            1, // SDDL_REVISION_1
            &mut sd,
            std::ptr::null_mut(),
        ) == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("转换安全描述符失败: {}", err));
        }

        let bin_dir_str = bin_dir.to_string_lossy().to_string();
        let bin_dir_w: Vec<u16> = bin_dir_str.encode_utf16().chain(std::iter::once(0)).collect();
        let res = SetFileSecurityW(bin_dir_w.as_ptr(), DACL_SECURITY_INFORMATION, sd);
        LocalFree(sd);

        if res == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("应用二进制目录安全属性失败: {}", err));
        }
    }
    Ok(())
}

/// 注册 Windows 系统服务 + 计划任务（服务运行模式）
///
/// 完整安装流程：
/// 1. 停止并删除已存在的同名服务（确保覆盖安装干净性）
/// 2. 复制二进制文件到 `%ProgramData%\Auroweave\bin`
/// 3. 对 bin 目录进行安全加锁
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

    // 步骤2-3: 复制二进制文件 + 对 bin 目录进行安全加锁
    let svc_path = copy_binaries(singbox_src_path)?;
    
    // 对 bin 目录进行加锁
    if let Some(parent) = svc_path.parent() {
        secure_bin_dir(parent)?;
    }

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
/// 1. 复制二进制文件到 `%ProgramData%\Auroweave\bin`
/// 2. 对 bin 目录进行安全加锁
/// 3. 生成 IPC 安全 Token
/// 4. 创建计划任务 `AuroweaveDirectTunTask`
pub fn install_task(singbox_src_path: Option<&str>) -> Result<(), String> {
    info!("开始以计划任务模式安装提权组件...");

    // 步骤1-2: 复制二进制文件 + 对 bin 目录进行安全加锁
    let svc_path = copy_binaries(singbox_src_path)?;
    
    // 对 bin 目录进行加锁
    if let Some(parent) = svc_path.parent() {
        secure_bin_dir(parent)?;
    }

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
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let bin_dir = PathBuf::from(program_data).join("Auroweave").join("bin");
    if bin_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&bin_dir) {
            info!("清理服务二进制目录失败（可能部分文件被占用）: {}", e);
        }
    }

    info!("系统服务及残留配置已彻底删除。");
    Ok(())
}

/// 生成并加密存储 IPC 安全 Token
///
/// 流程：
/// 1. 创建数据目录 `%ProgramData%\Auroweave\data`
/// 2. 若 Token 文件不存在则生成新 Token：
///    - 生成 UUID v4 作为 Token 明文
///    - 生成随机 Nonce（UUID 前 12 字节）
///    - 使用 AES-256-GCM 加密，拼接 Nonce + 密文写入文件
/// 3. 对 Token 文件设置安全描述符（SYSTEM/Admin 完全控制，Users 只读）
fn setup_token() -> Result<(), String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let data_dir = PathBuf::from(program_data).join("Auroweave").join("data");
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("创建数据目录失败: {}", e))?;

    let token_path = data_dir.join("ipc_token.bin");
    
    if !token_path.exists() {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm, Nonce,
        };
        const TOKEN_KEY: &[u8; 32] = b"AuroweaveIPCSecretKey2026_Secure";
        
        let token = uuid::Uuid::new_v4().to_string();
        
        let key: &aes_gcm::Key<Aes256Gcm> = TOKEN_KEY.into();
        let cipher = Aes256Gcm::new(key);
        
        let nonce_uuid = uuid::Uuid::new_v4();
        let nonce_bytes: [u8; 12] = nonce_uuid.as_bytes()[0..12].try_into().unwrap();
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, token.as_bytes())
            .map_err(|e| format!("加密 Token 失败: {:?}", e))?;
            
        let mut encrypted_data = nonce.to_vec();
        encrypted_data.extend_from_slice(&ciphertext);
        
        std::fs::write(&token_path, &encrypted_data).map_err(|e| format!("写入 Token 失败: {}", e))?;
        info!("生成新 Token 成功。");
    }

    let file_sddl = "D:(A;;FA;;;SY)(A;;FA;;;BA)(A;;FR;;;AU)";
    let file_sddl_w: Vec<u16> = file_sddl.encode_utf16().chain(std::iter::once(0)).collect();
    
    unsafe {
        let mut sd: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            file_sddl_w.as_ptr(),
            1,
            &mut sd,
            std::ptr::null_mut(),
        ) == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("转换文件安全描述符失败: {}", err));
        }

        let token_path_str = token_path.to_string_lossy().to_string();
        let token_path_w: Vec<u16> = token_path_str.encode_utf16().chain(std::iter::once(0)).collect();
        let res = SetFileSecurityW(token_path_w.as_ptr(), DACL_SECURITY_INFORMATION, sd);
        LocalFree(sd);

        if res == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("应用 Token 文件安全属性失败: {}", err));
        }
    }

    info!("已成功对 Token 文件进行安全锁定。");
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

/// 读取 Windows Machine GUID（HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid）
/// 每台机器唯一标识，用于派生 IPC Token 加密密钥
fn read_machine_guid() -> Option<String> {
let output = std::process::Command::new("reg")
    .args(["query", r"HKLM\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid"])
    .output()
    .ok()?;
let stdout = String::from_utf8_lossy(&output.stdout);
for line in stdout.lines() {
    if line.contains("MachineGuid") {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            return Some(parts[parts.len() - 1].to_string());
        }
    }
}
None
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
    let _ = std::fs::write("C:\\Users\\Xiang\\Desktop\\schtasks_create.log", format!("STDOUT:\n{}\nSTDERR:\n{}", out_str, err_str));

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
