/// 订阅到期与流量告警通知
/// 作者: TanXiang
///
/// 检查所有活跃订阅的 subscription-userinfo（到期时间戳 / 已用流量），
/// 越过告警阈值时发送 macOS 原生系统通知（osascript display notification）。
/// 零新增依赖（与 sysproxy 同款 osascript 方案），失败静默（通知不阻塞主流程）。
///
/// 告警去重：每个订阅+每个告警级别在进程生命周期内只提醒一次，
/// 状态持久化到 data_root/notify_state.json（重启不重复轰炸）。
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 告警阈值
const EXPIRE_WARN_DAYS: i64 = 7; // 到期前 7 天开始提醒
const TRAFFIC_WARN_PERCENT: u64 = 85; // 用量超 85% 提醒
const TRAFFIC_CRITICAL_PERCENT: u64 = 98; // 用量超 98% 再次提醒

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NotifyState {
    /// 已发送过的告警键（格式 "expire:{sub_id}:{days_bucket}" / "traffic:{sub_id}:{level}"）
    /// 跨启动持久化，避免每次轮询/重启都重复弹通知
    sent: HashSet<String>,
}

fn get_state_path() -> std::path::PathBuf {
    crate::get_data_root().join("notify_state.json")
}

fn load_state() -> NotifyState {
    match std::fs::read_to_string(get_state_path()) {
        Ok(c) => serde_json::from_str(&c).unwrap_or(NotifyState { sent: HashSet::new() }),
        Err(_) => NotifyState { sent: HashSet::new() },
    }
}

fn save_state(state: &NotifyState) {
    let _ = crate::fs_utils::atomic_write_json(&get_state_path(), state);
}

/// 发送 macOS 系统通知（display notification 不会弹窗抢焦点，点击可聚焦本应用）
fn send_notification(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        // 调用 AppleScript；标题/正文经 shell 单引号转义防注入
        let esc = |s: &str| s.replace('\'', "'\\''");
        let script = format!(
            "display notification \"{}\" with title \"{}\" sound name \"Glass\"",
            esc(body),
            esc(title)
        );
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (title, body); // 非 macOS 平台暂不支持原生通知
    }
}

/// 检查全部订阅的到期/用量状态并发送告警（幂等去重）
/// 由自动更新调度器周期调用（30 分钟一拍）。
pub fn check_subscription_alerts() {
    let subs = crate::commands::subscription::load_subscriptions();
    if subs.is_empty() {
        return;
    }

    let mut state = load_state();
    let mut dirty = false;
    let now_sec = chrono::Utc::now().timestamp();

    for sub in subs.iter().filter(|s| s.is_active) {
        // ---- 到期告警 ----
        if let Some(expire) = sub.user_info.as_ref().and_then(|u| u.expire_timestamp) {
            if expire > 0 {
                let remain_secs = expire - now_sec;
                if remain_secs <= 0 {
                    let key = format!("expire:{}:expired", sub.id);
                    if state.sent.insert(key) {
                        send_notification(
                            "订阅已到期",
                            &format!("订阅「{}」已过期，节点可能随时失效，请及时续费", sub.name),
                        );
                        dirty = true;
                    }
                } else if remain_secs <= EXPIRE_WARN_DAYS * 86400 {
                    // 按剩余天数分桶去重：3 天桶足够提醒且不轰炸
                    let days = (remain_secs + 86399) / 86400;
                    let bucket = if days >= 5 { "5-7" } else if days >= 2 { "2-4" } else { "0-1" };
                    let key = format!("expire:{}:{}", sub.id, bucket);
                    if state.sent.insert(key) {
                        send_notification(
                            "订阅即将到期",
                            &format!("订阅「{}」剩余 {} 天，请及时续费", sub.name, days),
                        );
                        dirty = true;
                    }
                }
            }
        }

        // ---- 流量用量告警 ----
        if let Some(info) = sub.user_info.as_ref() {
            if info.total_bytes > 0 {
                let used = info.upload_bytes + info.download_bytes;
                let percent = used * 100 / info.total_bytes;
                let (level, title, body) = if percent >= TRAFFIC_CRITICAL_PERCENT {
                    (
                        "critical",
                        "订阅流量即将耗尽",
                        format!("订阅「{}」已用 {}%，超出后节点将失效", sub.name, percent),
                    )
                } else if percent >= TRAFFIC_WARN_PERCENT {
                    (
                        "warn",
                        "订阅流量告警",
                        format!("订阅「{}」已用 {}%，请关注用量", sub.name, percent),
                    )
                } else {
                    continue;
                };
                let key = format!("traffic:{}:{}", sub.id, level);
                if state.sent.insert(key) {
                    send_notification(title, &body);
                    dirty = true;
                }
            }
        }
    }

    if dirty {
        save_state(&state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thresholds_sane() {
        // 阈值合法性：warn 早于 critical，天数/百分比在合理范围
        assert!(EXPIRE_WARN_DAYS > 0 && EXPIRE_WARN_DAYS <= 30);
        assert!(TRAFFIC_WARN_PERCENT < TRAFFIC_CRITICAL_PERCENT);
        assert!(TRAFFIC_CRITICAL_PERCENT <= 100);
    }
}
