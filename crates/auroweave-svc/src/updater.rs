/// AuroDaemon 自更新模块
/// 作者: TanXiang
///
/// 在服务/计划任务启动时自动检测并应用二进制更新。
/// 主程序在触发计划任务前会生成 `manifest.json`，包含最新的 AuroDaemon.exe 和
/// sing-box.exe 的路径与 SHA-256 哈希。服务启动时比对哈希，发现差异则自动更新。
///
/// 更新流程：
/// 1. 清理上次更新遗留的 `AuroDaemon.old.exe`
/// 2. 读取 `manifest.json` 获取期望的文件路径和哈希
/// 3. 比对 sing-box.exe 哈希，不一致则覆盖更新
/// 4. 比对 AuroDaemon.exe 自身哈希，不一致则自我更替：
///    a. 将自身重命名为 `AuroDaemon.old.exe`
///    b. 复制新版本到目标路径
///    c. 拉起新进程（传递相同参数）
///    d. 退出旧进程
/// 5. 复制失败时尝试回滚（恢复 old.exe）
use std::path::PathBuf;
use std::fs;
use std::process::Command;
use tracing::{info, warn};
use sha2::{Sha256, Digest};
use std::io::Read;

#[derive(serde::Deserialize)]
struct Manifest {
    svc_path: String,
    svc_hash: String,
    singbox_path: String,
    singbox_hash: String,
}

/// 计算文件的 SHA-256 哈希值（十六进制小写）
///
/// 使用 64KB 缓冲区读取文件，文件无法打开时返回空字符串。
fn compute_sha256(path: &std::path::Path) -> String {
    if let Ok(mut file) = fs::File::open(path) {
        let mut hasher = Sha256::new();
        // 扩大缓冲区至 64KB，显著提升大文件的磁盘 I/O 读取速度
        let mut buffer = [0; 65536];
        while let Ok(n) = file.read(&mut buffer) {
            if n == 0 { break; }
            hasher.update(&buffer[..n]);
        }
        let hash = hasher.finalize();
        hash.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    } else {
        String::new()
    }
}

/// 检查并应用二进制更新
///
/// 完整流程：
/// 1. 清理遗留的 `AuroDaemon.old.exe`（上次更新可能残留）
/// 2. 读取 `manifest.json`（不存在则跳过更新）
/// 3. 比对 sing-box.exe 哈希，不一致则覆盖更新
/// 4. 比对 AuroDaemon.exe 自身哈希，不一致则执行自我更替
pub fn check_and_apply_updates() {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let config_dir = PathBuf::from(&program_data).join("Auroweave").join("config");
    let bin_dir = PathBuf::from(&program_data).join("Auroweave").join("bin");
    
    // ---- 步骤1: 清理上次更新遗留的 old.exe ----
    let old_svc_path = bin_dir.join("AuroDaemon.old.exe");
    if old_svc_path.exists() {
        if let Err(e) = fs::remove_file(&old_svc_path) {
            warn!("无法删除遗留的 old.exe: {}", e);
        } else {
            info!("成功清理遗留的 AuroDaemon.old.exe");
        }
    }
    
    // ---- 步骤2: 读取 manifest.json（不存在则跳过） ----
    let manifest_path = config_dir.join("manifest.json");
    if !manifest_path.exists() {
        return;
    }
    
    let content = match fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    
    let manifest: Manifest = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            warn!("解析 manifest.json 失败: {}", e);
            return;
        }
    };
    
    // ---- 步骤3: 检查 sing-box.exe 是否需要更新 ----
    // 比对当前文件的 SHA-256 与 manifest 中的期望哈希
    let target_sb_path = bin_dir.join("sing-box.exe");
    let current_sb_hash = compute_sha256(&target_sb_path);
    if current_sb_hash != manifest.singbox_hash {
        info!("发现 sing-box.exe 更新 ({} != {})，正在覆盖...", current_sb_hash, manifest.singbox_hash);
        let src_sb = PathBuf::from(&manifest.singbox_path);
        if src_sb.exists() {
            if let Err(e) = fs::copy(&src_sb, &target_sb_path) {
                warn!("覆盖 sing-box.exe 失败: {}", e);
            } else {
                info!("sing-box.exe 更新完成");
            }
        }
    }
    
    // ---- 步骤4: 检查 AuroDaemon.exe 自身是否需要更新 ----
    // 若哈希不一致，执行自我更替：重命名 → 复制 → 拉起新进程 → 退出旧进程
    let target_svc_path = std::env::current_exe().unwrap_or_else(|_| bin_dir.join("AuroDaemon.exe"));
    let current_svc_hash = compute_sha256(&target_svc_path);
    if current_svc_hash != manifest.svc_hash {
        info!("发现 AuroDaemon.exe 更新，准备自我更替...");
        let src_svc = PathBuf::from(&manifest.svc_path);
        if src_svc.exists() {
            // 步骤4a: 重命名自身为 old.exe（正在运行的 exe 文件无法直接覆盖）
            if let Err(e) = fs::rename(&target_svc_path, &old_svc_path) {
                warn!("重命名自身失败，跳过本次更新: {}", e);
                return;
            }
            
            // 步骤4b: 复制新版本到目标路径
            if let Err(e) = fs::copy(&src_svc, &target_svc_path) {
                warn!("拷贝新版本 AuroDaemon.exe 失败: {}", e);
                // 尝试回滚
                let _ = fs::rename(&old_svc_path, &target_svc_path);
                return;
            }
            
            info!("AuroDaemon.exe 自我更替成功，拉起新进程并退出旧进程");
            
            // 步骤4c: 拉起新进程并退出旧进程
            // 传递相同的命令行参数，确保新进程以相同模式运行
            let args: Vec<String> = std::env::args().skip(1).collect();
            let mut cmd = Command::new(&target_svc_path);
            cmd.args(&args);
            
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            
            if let Err(e) = cmd.spawn() {
                warn!("拉起新进程失败: {}", e);
                return; // 如果失败，就不退出了，硬着头皮继续跑
            }
            
            // 成功拉起，立刻退出老进程
            std::process::exit(0);
        }
    }
}
