//! Daemon 内置的轻量级 cron-style scheduler。
//!
//! 每小时唤醒一次（启动后 60s 冷却 + 1h interval），检查两件事：
//!
//! 1. **weekly_report**：周日 22:00 ~ 22:59 间，今天还没生成过 → 调
//!    [`regenerate_weekly_report`] + 写一条 `KIND_WEEKLY_REPORT` 通知
//! 2. **reflect_pending**：每天最多一次，扫描 "24h 前的 session 中还没生成
//!    L2 摘要" 的数量 > 0 时写一条 `KIND_REFLECT_PENDING` 通知（前提：用户
//!    没在 Settings 关掉这个开关）
//!
//! 两件事都受 [`Db::notification_enabled`] 控制，跟 ingest_failed 一致；
//! 触发后用 `kv` 表里的 `notify.<kind>.last_date` 记录"今天已经触发过"，避免
//! scheduler 反复唤醒 → 反复写同一条通知。
//!
//! 时间戳一律用本地时区 (`chrono::Local`)，跟用户看到的 "周日 22:00" 文案一致。
//!
//! 失败处理：任何一步出错都只打 warn 日志，不影响 daemon 主流程；下一小时再
//! 尝试。LLM provider 未配置时 weekly_report 静默跳过（不打扰用户）。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Datelike, Local, Timelike, Weekday};
use memex_core::config::MemexConfig;
use memex_core::ingest::regenerate_weekly_report;
use memex_core::llm::select_provider_unified;
use memex_core::storage::db::Db;
use memex_core::storage::notifications::{KIND_REFLECT_PENDING, KIND_WEEKLY_REPORT};
use tracing::{info, warn};

/// scheduler tick 间隔。1 小时足够 weekly_report （触发窗口是整个 22:00 时段）
/// 和 reflect_pending （每天一次）的精度需求。
const TICK_INTERVAL: Duration = Duration::from_secs(3600);

/// 启动后冷却时间。让 daemon 先把 bootstrap ingest 跑完再开始 tick，
/// 避免抢 cpu / db 锁。
const STARTUP_DELAY: Duration = Duration::from_secs(60);

/// "反思待处理" 语义：超过 24h 前 ingest 的 session 中还没生成 L2 摘要的数量。
/// 用户视角是"留了一堆没复盘的会话"。
const REFLECT_STALE_HOURS: i64 = 24;

/// `regenerate_weekly_report` 触发的窗口（每周日的几点到几点）。
const WEEKLY_REPORT_HOUR: u32 = 22;

/// 启动 scheduler 后台 task。返回后不等待 —— task 在 tokio runtime 里独立运行，
/// daemon 退出时 tokio 会自动 drop 该 task（跟 watcher 一样的生命周期模型）。
pub fn start_scheduler(db: Arc<Db>, memex_dir: PathBuf) {
    tokio::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        let mut interval = tokio::time::interval(TICK_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        info!("scheduler: started (interval={}s)", TICK_INTERVAL.as_secs());

        loop {
            interval.tick().await;
            run_once(&db, &memex_dir);
        }
    });
}

fn run_once(db: &Db, memex_dir: &Path) {
    let now = Local::now();

    if let Err(e) = check_weekly_report(db, memex_dir, now) {
        warn!("scheduler: weekly_report check failed: {}", e);
    }

    if let Err(e) = check_reflect_pending(db, now) {
        warn!("scheduler: reflect_pending check failed: {}", e);
    }
}

/// 周日 22:00 触发：先看用户开关 → 看本周日是否已触发过 → 跑 regenerate → 写通知。
///
/// 容错：
/// * LLM provider 未配置：静默跳过，下周再试（用户开 LLM 后自然就有了）
/// * regenerate 失败：写 warn 日志但仍然把 last_date 标成今天，避免反复重试
///   占满日志（用户可以手动 "重新生成周报" 按钮再走一次）
fn check_weekly_report(db: &Db, memex_dir: &Path, now: DateTime<Local>) -> Result<()> {
    if !db.notification_enabled(KIND_WEEKLY_REPORT) {
        return Ok(());
    }
    if now.weekday() != Weekday::Sun || now.hour() != WEEKLY_REPORT_HOUR {
        return Ok(());
    }

    let today_key = now.format("%Y-%m-%d").to_string();
    if db.kv_get(&kv_last_date_key(KIND_WEEKLY_REPORT))?
        .as_deref()
        == Some(today_key.as_str())
    {
        return Ok(());
    }

    // 用户没配 LLM 就 skip：scheduler 不该主动用 fallback / mock provider
    // 生成假数据，那是糟糕的用户体验。下周再试。
    let cfg = MemexConfig::load(memex_dir)?;
    let Some(provider) = select_provider_unified(db, &cfg.llm, memex_dir) else {
        info!("scheduler: weekly_report skipped (no LLM provider configured)");
        return Ok(());
    };

    match regenerate_weekly_report(db, provider.as_ref()) {
        Ok(Some(_row)) => {
            let _ = db.insert_notification(
                KIND_WEEKLY_REPORT,
                "周报已生成",
                "本周的工作总结已就绪，点击查看。",
                Some(r#"{"trigger":"scheduler"}"#),
            );
            info!("scheduler: weekly_report generated for {}", today_key);
        }
        Ok(None) => {
            info!("scheduler: weekly_report has no data this week");
        }
        Err(e) => {
            warn!("scheduler: weekly_report generation failed: {}", e);
        }
    }
    // 不管 LLM 成功失败都打 last_date：避免重试占资源
    db.kv_set(&kv_last_date_key(KIND_WEEKLY_REPORT), &today_key)?;
    Ok(())
}

/// 每天最多触发一次。条件：当前有 `count_stale_unsummarized_sessions() > 0`
/// （= 至少有 1 个 session 超过 24h 还没生成 L2 summary）。
fn check_reflect_pending(db: &Db, now: DateTime<Local>) -> Result<()> {
    if !db.notification_enabled(KIND_REFLECT_PENDING) {
        return Ok(());
    }

    let today_key = now.format("%Y-%m-%d").to_string();
    if db.kv_get(&kv_last_date_key(KIND_REFLECT_PENDING))?
        .as_deref()
        == Some(today_key.as_str())
    {
        return Ok(());
    }

    let count = db.count_sessions_without_summary_older_than(REFLECT_STALE_HOURS)?;
    if count > 0 {
        let payload = serde_json::json!({
            "pending": count,
            "stale_hours": REFLECT_STALE_HOURS,
        })
        .to_string();
        let _ = db.insert_notification(
            KIND_REFLECT_PENDING,
            "反思待处理",
            &format!(
                "有 {} 个会话超过 {} 小时未生成摘要，可能值得复盘。",
                count, REFLECT_STALE_HOURS
            ),
            Some(&payload),
        );
        info!(
            "scheduler: reflect_pending notification raised (count={})",
            count
        );
    }
    // 即使 count == 0 也标记今天处理过 —— 否则会从早 8 点一直查到晚 8 点
    db.kv_set(&kv_last_date_key(KIND_REFLECT_PENDING), &today_key)?;
    Ok(())
}

/// `pref.notify.<kind>` 是用户开关；`notify.<kind>.last_date` 是触发频次记账。
/// 命名 prefix 不重叠，方便用户在 db 里 grep。
fn kv_last_date_key(kind: &str) -> String {
    format!("notify.{}.last_date", kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_local(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .expect("valid local datetime")
    }

    /// 这个测试的目的不是验证 chrono 周几判断，而是把"什么时间会触发周报"这条
    /// 业务规则固定下来：只有周日 22:xx 才进 LLM 调用分支。改动这条规则时
    /// 测试必须同步改。
    #[test]
    fn weekly_report_only_triggers_on_sunday_22() {
        // 2026-06-14 是周日
        let sunday_22 = make_local(2026, 6, 14, 22);
        let sunday_23 = make_local(2026, 6, 14, 23);
        let monday_22 = make_local(2026, 6, 15, 22);
        let sunday_21 = make_local(2026, 6, 14, 21);

        assert_eq!(sunday_22.weekday(), Weekday::Sun);
        assert!(sunday_22.weekday() == Weekday::Sun && sunday_22.hour() == WEEKLY_REPORT_HOUR);
        assert!(!(sunday_23.weekday() == Weekday::Sun && sunday_23.hour() == WEEKLY_REPORT_HOUR));
        assert!(!(monday_22.weekday() == Weekday::Sun && monday_22.hour() == WEEKLY_REPORT_HOUR));
        assert!(!(sunday_21.weekday() == Weekday::Sun && sunday_21.hour() == WEEKLY_REPORT_HOUR));
    }

    #[test]
    fn weekly_report_skipped_when_switch_off() {
        let db = Db::open_in_memory().unwrap();
        db.kv_set("pref.notify.weekly_report", "false").unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let now = make_local(2026, 6, 14, 22);
        check_weekly_report(&db, tmp.path(), now).unwrap();
        assert!(
            db.kv_get(&kv_last_date_key(KIND_WEEKLY_REPORT))
                .unwrap()
                .is_none()
        );
        assert_eq!(db.count_unread_notifications().unwrap(), 0);
    }

    #[test]
    fn weekly_report_idempotent_within_same_day() {
        let db = Db::open_in_memory().unwrap();
        let today_key = make_local(2026, 6, 14, 22).format("%Y-%m-%d").to_string();
        db.kv_set(&kv_last_date_key(KIND_WEEKLY_REPORT), &today_key)
            .unwrap();
        let tmp = tempfile::tempdir().unwrap();

        let now = make_local(2026, 6, 14, 22);
        check_weekly_report(&db, tmp.path(), now).unwrap();
        assert_eq!(db.count_unread_notifications().unwrap(), 0);
    }

    #[test]
    fn reflect_pending_skipped_when_switch_off() {
        let db = Db::open_in_memory().unwrap();
        db.kv_set("pref.notify.reflect_pending", "false").unwrap();
        let now = make_local(2026, 6, 14, 9);
        check_reflect_pending(&db, now).unwrap();
        assert!(
            db.kv_get(&kv_last_date_key(KIND_REFLECT_PENDING))
                .unwrap()
                .is_none()
        );
        assert_eq!(db.count_unread_notifications().unwrap(), 0);
    }

    #[test]
    fn reflect_pending_marks_last_date_even_when_count_is_zero() {
        // 空库：count == 0，但要把 last_date 打上避免一天内反复扫
        let db = Db::open_in_memory().unwrap();
        let now = make_local(2026, 6, 14, 9);
        check_reflect_pending(&db, now).unwrap();
        let last = db.kv_get(&kv_last_date_key(KIND_REFLECT_PENDING)).unwrap();
        assert_eq!(last.as_deref(), Some("2026-06-14"));
        assert_eq!(db.count_unread_notifications().unwrap(), 0);
    }

}
