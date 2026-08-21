/// Sing-box 内核版本检测、在线升级与热替换模块
/// 作者: TanXiang
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;
use tracing::info;


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingboxUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_notes: String,
    pub published_at: String,
    pub download_url: Option<String>,
    pub download_size: u64,
}

/// GitHub Release API 响应数据结构
#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseResponse {
    tag_name: String,
    body: Option<String>,
    published_at: Option<String>,
    assets: Vec<GithubReleaseAsset>,
}

/// 获取当前已安装的 sing-box 版本号
pub fn get_current_singbox_version() -> String {
    if let Ok(path) = crate::core::sidecar::SidecarManager::resolve_binary_path() {
        if let Ok(output) = std::process::Command::new(path).arg("version").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.starts_with("sing-box version ") {
                    return line.replace("sing-box version ", "").trim().to_string();
                }
            }
            if let Some(first) = stdout.lines().next() {
                return first.trim().to_string();
            }
        }
    }
    "未知版本".to_string()
}

/// 检查 Sing-box GitHub 官方最新 Release
#[tauri::command]
pub async fn core_check_singbox_update() -> Result<ApiResponse<SingboxUpdateInfo>, AppError> {
    let current_ver = get_current_singbox_version();
    info!("当前 sing-box 版本: {}", current_ver);

    let client = reqwest::Client::builder()
        .user_agent("Auroweave-Client/1.0")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| AppError::Network(format!("初始化 HTTP 客户端失败: {}", e)))?;

    let resp = client
        .get("https://api.github.com/repos/SagerNet/sing-box/releases/latest")
        .send()
        .await
        .map_err(|e| AppError::Network(format!("请求 GitHub Release 失败: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Network(format!("GitHub API 返回错误状态码: {}", resp.status())));
    }

    let release: GithubReleaseResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Network(format!("解析 Release 数据失败: {}", e)))?;

    let latest_ver = release.tag_name.trim_start_matches('v').to_string();
    let clean_current = current_ver.trim_start_matches('v').to_string();

    let has_update = !clean_current.is_empty()
        && clean_current != "未知版本"
        && clean_current != latest_ver;

    // 匹配当前平台架构的资产名称
    let target_keyword = get_platform_asset_keyword();
    let mut matched_url = None;
    let mut matched_size = 0;

    for asset in release.assets {
        if asset.name.contains(&target_keyword) {
            matched_url = Some(asset.browser_download_url);
            matched_size = asset.size;
            break;
        }
    }

    let info = SingboxUpdateInfo {
        current_version: current_ver,
        latest_version: release.tag_name,
        has_update,
        release_notes: release.body.unwrap_or_default(),
        published_at: release.published_at.unwrap_or_default(),
        download_url: matched_url,
        download_size: matched_size,
    };

    Ok(ApiResponse::ok(info))
}

fn get_platform_asset_keyword() -> String {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "darwin-arm64.tar.gz".to_string();

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "darwin-amd64.tar.gz".to_string();

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "windows-amd64.zip".to_string();

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "linux-amd64.tar.gz".to_string();

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "linux-arm64.tar.gz".to_string();

    #[allow(unreachable_code)]
    "tar.gz".to_string()
}

/// 执行内核在线下载、解压并替换升级
#[tauri::command]
pub async fn core_upgrade_singbox(
    app_handle: AppHandle,
    download_url: String,
) -> Result<ApiResponse<()>, AppError> {
    info!("开始下载 sing-box 内核升级包: {}", download_url);

    let client = reqwest::Client::builder()
        .user_agent("Auroweave-Client/1.0")
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| AppError::Network(format!("初始化下载客户端失败: {}", e)))?;

    let resp = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| AppError::Network(format!("下载内核升级包失败: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::Network(format!("下载返回异常状态: {}", resp.status())));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Network(format!("读取升级包数据流失败: {}", e)))?;

    let temp_dir = std::env::temp_dir().join("auroweave_update");
    let _ = std::fs::create_dir_all(&temp_dir);

    let archive_path = if download_url.ends_with(".zip") {
        temp_dir.join("singbox_update.zip")
    } else {
        temp_dir.join("singbox_update.tar.gz")
    };

    std::fs::write(&archive_path, &bytes)
        .map_err(|e| AppError::Io(format!("保存临时升级包失败: {}", e)))?;

    info!("升级包已写入临时文件: {:?}，开始解压", archive_path);

    // 解压到 temp_dir/extracted
    let extract_dir = temp_dir.join("extracted");
    let _ = std::fs::remove_dir_all(&extract_dir);
    let _ = std::fs::create_dir_all(&extract_dir);

    #[cfg(not(target_os = "windows"))]
    {
        let status = std::process::Command::new("tar")
            .arg("-xzf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&extract_dir)
            .status()
            .map_err(|e| AppError::Io(format!("解压 tar.gz 失败: {}", e)))?;

        if !status.success() {
            return Err(AppError::Sidecar("tar 解压升级包返回错误退出码".to_string()));
        }
    }

    #[cfg(target_os = "windows")]
    {
        let status = std::process::Command::new("tar")
            .arg("-xf")
            .arg(&archive_path)
            .arg("-C")
            .arg(&extract_dir)
            .status()
            .map_err(|e| AppError::Io(format!("解压 zip 失败: {}", e)))?;

        if !status.success() {
            return Err(AppError::Sidecar("tar 解压 zip 升级包返回错误退出码".to_string()));
        }
    }

    // 在 extract_dir 中递归寻找 sing-box 二进制文件
    let mut found_bin: Option<PathBuf> = None;
    for entry in walkdir(&extract_dir) {
        let name = entry.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        #[cfg(target_os = "windows")]
        if name == "sing-box.exe" {
            found_bin = Some(entry);
            break;
        }
        #[cfg(not(target_os = "windows"))]
        if name == "sing-box" {
            found_bin = Some(entry);
            break;
        }
    }

    let source_bin = found_bin.ok_or_else(|| AppError::Sidecar("解压包内未找到 sing-box 可执行文件".to_string()))?;
    info!("找到新版本 sing-box 二进制文件: {:?}", source_bin);

    // 确定目标安装目录
    #[cfg(target_os = "windows")]
    let target_bin = {
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
        let bin_dir = PathBuf::from(program_data).join("Auroweave").join("bin");
        let _ = std::fs::create_dir_all(&bin_dir);
        bin_dir.join("sing-box.exe")
    };

    #[cfg(target_os = "macos")]
    let target_bin = {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let bin_dir = PathBuf::from(home).join("Library").join("Application Support").join("Auroweave").join("bin");
        let _ = std::fs::create_dir_all(&bin_dir);
        bin_dir.join("sing-box")
    };

    #[cfg(target_os = "linux")]
    let target_bin = {
        let bin_dir = crate::get_data_root().join("bin");
        let _ = std::fs::create_dir_all(&bin_dir);
        bin_dir.join("sing-box")
    };

    // 替换目标文件
    info!("将新内核复制到目标路径: {:?}", target_bin);
    std::fs::copy(&source_bin, &target_bin)
        .map_err(|e| AppError::Io(format!("复制内核文件失败: {}", e)))?;


    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&target_bin) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&target_bin, perms);
        }
    }

    // 清理临时文件
    let _ = std::fs::remove_dir_all(&temp_dir);

    // 触发内核平滑重启
    let _ = crate::system::startup::apply_core_mode_with_fallback(&app_handle).await;
    info!("sing-box 内核升级完成并已重新拉起运行");

    Ok(ApiResponse::ok(()))
}

fn walkdir(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir(&path));
            } else {
                files.push(path);
            }
        }
    }
    files
}
