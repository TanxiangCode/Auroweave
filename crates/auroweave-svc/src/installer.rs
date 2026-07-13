use std::path::PathBuf;
use tracing::info;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW;
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, DACL_SECURITY_INFORMATION};
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, ControlService, CreateServiceW, DeleteService, OpenSCManagerW, OpenServiceW, SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS, SERVICE_CONTROL_STOP, SERVICE_DEMAND_START, SERVICE_ERROR_NORMAL, SERVICE_STATUS, SERVICE_WIN32_OWN_PROCESS
};
use windows_sys::Win32::Security::SetFileSecurityW;

const ERROR_SERVICE_MARKED_FOR_DELETE: u32 = 1072;
const ERROR_SERVICE_ALREADY_EXISTS: u32 = 1073;

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

pub fn install() -> Result<(), String> {
    info!("开始安装 Windows 系统服务...");

    let exe_path = std::env::current_exe().map_err(|e| format!("获取自身路径失败: {}", e))?;
    // 需要用双引号包裹路径以防止空格截断安全漏洞
    let binary_path_str = format!("\"{}\" run", exe_path.to_string_lossy());
    let binary_path_w: Vec<u16> = binary_path_str.encode_utf16().chain(std::iter::once(0)).collect();

    let service_name_w: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
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
            let raw_err = err.raw_os_error();
            if raw_err == Some(ERROR_SERVICE_ALREADY_EXISTS as i32) {
                info!("服务已存在，开始更新配置与安全描述符...");
                return update_existing_service();
            }
            if raw_err == Some(ERROR_SERVICE_MARKED_FOR_DELETE as i32) {
                return Err("服务项已被 Windows 标记为‘待删除(Delete Pending)’状态。请关闭 Windows 服务管理器 (services.msc) 或任务管理器以彻底释放句柄，然后于 3 秒后重试。".to_string());
            }
            return Err(format!("创建服务项失败: {}", err));
        }

        info!("系统服务项已创建成功，正在设置安全描述符...");

        // 设置服务的安全描述符（DACL）
        // 允许 SYSTEM (SY) 和 Administrators (BA) 完全控制
        // 允许 Authenticated Users (AU) 拥有: CC(QueryConfig), LC(QueryStatus), SW(Start), LO(Stop), RC(ReadControl) 权限，免 UAC 启停
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

    info!("系统服务配置成功。正在生成 IPC Token...");
    setup_token()?;

    info!("服务安装与安全加固已全部完成。");
    Ok(())
}

fn update_existing_service() -> Result<(), String> {
    let service_name_w: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm == 0 {
            return Err("打开 SCM 失败".to_string());
        }
        let service = OpenServiceW(scm, service_name_w.as_ptr(), SERVICE_ALL_ACCESS);
        if service == 0 {
            CloseServiceHandle(scm);
            return Err("打开现有服务失败".to_string());
        }

        // 重新设置安全描述符
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
            return Err(format!("更新转换 SDDL 失败: {}", err));
        }

        let res = SetServiceObjectSecurity(service, DACL_SECURITY_INFORMATION, sd);
        LocalFree(sd);

        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        if res == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("更新服务安全描述符失败: {}", err));
        }
    }

    setup_token()?;
    info!("服务更新与安全加固已完成。");
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    info!("开始卸载 Windows 系统服务...");
    
    let service_name_w: Vec<u16> = SERVICE_NAME.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_ALL_ACCESS);
        if scm == 0 {
            return Err("打开 SCM 失败，请确认管理员权限".to_string());
        }

        let service = OpenServiceW(scm, service_name_w.as_ptr(), SERVICE_ALL_ACCESS);
        if service == 0 {
            CloseServiceHandle(scm);
            // 服务本身可能就未安装，属于正常
            info!("未检测到已安装的服务，清理相关残留文件...");
            let _ = cleanup_token();
            return Ok(());
        }

        // 先停止服务
        let mut status: SERVICE_STATUS = std::mem::zeroed();
        let _ = ControlService(service, SERVICE_CONTROL_STOP, &mut status);

        // 删除服务
        let res = DeleteService(service);
        CloseServiceHandle(service);
        CloseServiceHandle(scm);

        if res == 0 {
            let err = std::io::Error::last_os_error();
            return Err(format!("从系统删除服务失败: {}", err));
        }
    }

    let _ = cleanup_token();
    info!("系统服务及残留配置已彻底删除。");
    Ok(())
}

fn setup_token() -> Result<(), String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let cache_dir = PathBuf::from(program_data).join("Auroweave");
    std::fs::create_dir_all(&cache_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let token_path = cache_dir.join("token.txt");
    
    // 如果 token 已经存在则不需要重新生成，防止在覆盖安装时导致 GUI 和服务不同步
    if !token_path.exists() {
        let token = uuid::Uuid::new_v4().to_string();
        std::fs::write(&token_path, &token).map_err(|e| format!("写入 Token 失败: {}", e))?;
        info!("生成新 Token 成功。");
    }

    // 设置 token.txt 文件权限：
    // D:(A;;FA;;;SY)(A;;FA;;;BA)(A;;FR;;;AU)
    // SYSTEM(SY) 和 Administrators(BA) 拥有完全控制，Authenticated Users(AU) 仅有只读权限
    let file_sddl = "D:(A;;FA;;;SY)(A;;FA;;;BA)(A;;FR;;;AU)";
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

fn cleanup_token() -> Result<(), String> {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let token_path = PathBuf::from(program_data).join("Auroweave").join("token.txt");
    if token_path.exists() {
        let _ = std::fs::remove_file(token_path);
    }
    Ok(())
}
