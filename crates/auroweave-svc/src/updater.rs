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

pub fn check_and_apply_updates() {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let config_dir = PathBuf::from(&program_data).join("Auroweave").join("config");
    let bin_dir = PathBuf::from(&program_data).join("Auroweave").join("bin");
    
    // 1. 清理遗留的 .old.exe
    let old_svc_path = bin_dir.join("auroweave-svc.old.exe");
    if old_svc_path.exists() {
        if let Err(e) = fs::remove_file(&old_svc_path) {
            warn!("无法删除遗留的 old.exe: {}", e);
        } else {
            info!("成功清理遗留的 auroweave-svc.old.exe");
        }
    }
    
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
    
    // 2. 检查 sing-box.exe 是否有更新
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
    
    // 3. 检查 AuroDaemon.exe 本身是否有更新
    let target_svc_path = std::env::current_exe().unwrap_or_else(|_| bin_dir.join("AuroDaemon.exe"));
    let current_svc_hash = compute_sha256(&target_svc_path);
    if current_svc_hash != manifest.svc_hash {
        info!("发现 AuroDaemon.exe 更新，准备自我更替...");
        let src_svc = PathBuf::from(&manifest.svc_path);
        if src_svc.exists() {
            // 尝试重命名自己
            if let Err(e) = fs::rename(&target_svc_path, &old_svc_path) {
                warn!("重命名自身失败，跳过本次更新: {}", e);
                return;
            }
            
            // 拷贝新版本
            if let Err(e) = fs::copy(&src_svc, &target_svc_path) {
                warn!("拷贝新版本 AuroDaemon.exe 失败: {}", e);
                // 尝试回滚
                let _ = fs::rename(&old_svc_path, &target_svc_path);
                return;
            }
            
            info!("AuroDaemon.exe 自我更替成功，拉起新进程并退出旧进程");
            
            // 拉起新进程 (注意传递相同参数)
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
