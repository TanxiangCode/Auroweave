/// Sing-box 内核版本检测、在线升级与热替换模块
/// 作者: TanXiang
use crate::error::{ApiResponse, AppError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
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

/// 内核升级包下载来源白名单（域名后缀匹配）
///
/// 只允许 GitHub 官方 Release 域名，防止任意 URL 被传入下载二进制并替换内核
/// （内核二进制以 SUID/管理员权限运行，被替换意味着任意代码提权执行）。
const ALLOWED_DOWNLOAD_HOSTS: [&str; 4] = [
    "github.com",
    "objects.githubusercontent.com",
    "github-releases.githubusercontent.com",
    "api.github.com",
];

/// 校验下载 URL 的 host 是否在白名单内（精确匹配或子域名后缀匹配）
fn validate_download_url(url_str: &str) -> Result<(), AppError> {
    let parsed = url::Url::parse(url_str)
        .map_err(|e| AppError::Validation(format!("无效的下载链接: {}", e)))?;
    // 仅允许 HTTPS（防中间人替换二进制）
    if parsed.scheme() != "https" {
        return Err(AppError::Validation("下载链接必须使用 HTTPS".to_string()));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::Validation("下载链接缺少主机名".to_string()))?;
    let is_allowed = ALLOWED_DOWNLOAD_HOSTS
        .iter()
        .any(|allowed| host == *allowed || host.ends_with(&format!(".{}", allowed)));
    if is_allowed {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "不允许的下载来源: {}（仅支持 GitHub 官方域名）",
            host
        )))
    }
}

/// 计算 SHA-256（十六进制小写）
fn compute_sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// 执行内核在线下载、解压并替换升级
///
/// 安全与稳定性约束：
/// 1. 下载来源域名白名单（仅 GitHub 官方 Release 域）+ 强制 HTTPS
/// 2. 可选 sha256 参数：提供时下载后校验哈希，不匹配则中止（不会替换现有内核）
/// 3. 升级前若内核正在运行，先停止内核再复制，避免目标文件被占用导致升级必然失败
/// 4. 临时目录带进程唯一后缀（auroweave_update_{uuid}），避免并发实例互相干扰
#[tauri::command]
pub async fn core_upgrade_singbox(
    app_handle: AppHandle,
    download_url: String,
    sha256: Option<String>,
) -> Result<ApiResponse<()>, AppError> {
    // 0. 下载来源白名单校验（先于任何网络请求）
    validate_download_url(&download_url)?;
    info!("开始下载 sing-box 内核升级包: {}", download_url);

    let client = reqwest::Client::builder()
        .user_agent("Auroweave-Client/1.0")
        // 大版本内核包体积可达数十 MB，慢速网络下 120s 不够；
        // 600s 兼顾慢速网络与异常卡死（连接级别失败由系统 TCP 超时兜底）
        .timeout(std::time::Duration::from_secs(600))
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

    // 1. 可选 SHA-256 校验：不匹配直接中止，不影响现有内核
    if let Some(expected) = sha256.as_deref() {
        let expected = expected.trim().to_lowercase();
        let actual = compute_sha256_hex(&bytes);
        if actual != expected {
            return Err(AppError::Validation(format!(
                "升级包 SHA-256 校验失败: 期望 {}，实际 {}",
                expected, actual
            )));
        }
        info!("升级包 SHA-256 校验通过: {}", actual);
    }

    // 2. 进程唯一临时目录，避免并发升级实例共用目录互相覆盖
    let temp_dir = std::env::temp_dir()
        .join(format!("auroweave_update_{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| AppError::Io(format!("创建临时目录失败: {}", e)))?;

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

    // 升级前先停止运行中的内核：否则复制目标被占用必然失败
    // （Windows 上目标文件被运行中进程锁定；Unix 上即使 rename 成功，运行中的也是旧 inode）
    {
        let running_res = crate::commands::settings::core_query_running(app_handle.clone()).await;
        let is_running = running_res.data.unwrap_or(false);
        if is_running {
            info!("[upgrade] 检测到内核正在运行，先停止内核再替换二进制...");
            let sm = app_handle
                .state::<std::sync::Arc<crate::core::sidecar::SidecarManager>>()
                .inner()
                .clone();
            if let Err(e) = sm.stop().await {
                log::warn!("[upgrade] 停止旧内核失败（继续尝试复制，若被占用将返回明确错误）: {}", e);
            }
            // 给操作系统一点时间释放文件句柄
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
    }

    // 替换目标文件：失败时给出明确指引并清理临时目录
    info!("将新内核复制到目标路径: {:?}", target_bin);
    if let Err(e) = std::fs::copy(&source_bin, &target_bin) {
        // 复制失败最常见原因是目标仍被运行中的内核/服务占用
        let _ = std::fs::remove_dir_all(&temp_dir);
        return Err(AppError::Io(format!(
            "复制内核文件失败: {}。请先在设置中停止内核/系统服务后再升级",
            e
        )));
    }


    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&target_bin) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&target_bin, perms);
        }

        // macOS TUN 模式依赖 SUID root：fs::copy 以新建文件方式写入，SUID 位会丢失，
        // 升级后需要重新提权设置（未设置时仅告警，TUN 模式下次启动会再次请求授权）
        #[cfg(target_os = "macos")]
        {
            if !crate::core::sidecar::SidecarManager::is_privileged_binary(&target_bin) {
                log::warn!(
                    "[upgrade] 新内核尚未具备 SUID root 权限，TUN 模式下次启动时可能再次请求管理员授权"
                );
            }
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
