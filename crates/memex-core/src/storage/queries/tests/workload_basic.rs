//! Workload aggregations and time-window filtering. Heatmap-specific
//! and daily-double-count tests live in sibling files.

use super::ws_seed_session;
use crate::storage::db::Db;

#[test]
fn test_workload_report_empty() {
    let db = Db::open_in_memory().unwrap();
    let r = db.workload_report(7).unwrap();
    assert_eq!(r.days, 7);
    assert_eq!(r.overall.sessions, 0);
    assert_eq!(r.overall.active_days, 0);
    assert!(r.daily.is_empty());
    assert!(r.by_adapter.is_empty());
    assert!(r.by_project.is_empty());
    assert!(r.heatmap.is_empty());
    assert!(r.overall.peak_day.is_none());
}

#[test]
fn test_workload_report_aggregations() {
    let db = Db::open_in_memory().unwrap();
    // 用今天的本地时间，避免 cutoff (DATE('now','localtime','-N days')) 把数据剪掉。
    let now = chrono::Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let yesterday = (now - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    // 4 个 session：
    //   2 个 claude_code @ /a (今天)
    //   1 个 cursor      @ /a (昨天)
    //   1 个 cursor      @ /b (今天)
    ws_seed_session(
        &db,
        "s1",
        "claude_code",
        Some("/a"),
        &format!("{today}T10:00:00+08:00"),
        10,
    );
    ws_seed_session(
        &db,
        "s2",
        "claude_code",
        Some("/a"),
        &format!("{today}T15:00:00+08:00"),
        20,
    );
    ws_seed_session(
        &db,
        "s3",
        "cursor",
        Some("/a"),
        &format!("{yesterday}T11:00:00+08:00"),
        30,
    );
    ws_seed_session(
        &db,
        "s4",
        "cursor",
        Some("/b"),
        &format!("{today}T09:00:00+08:00"),
        5,
    );

    let r = db.workload_report(30).unwrap();
    assert_eq!(r.overall.sessions, 4);
    assert_eq!(r.overall.messages, 65);
    assert!(r.overall.active_days >= 1);

    let cc = r
        .by_adapter
        .iter()
        .find(|b| b.key == "claude_code")
        .unwrap();
    assert_eq!(cc.sessions, 2);
    assert_eq!(cc.messages, 30);

    let proj_a = r
        .by_project
        .iter()
        .find(|p| p.project_path == "/a")
        .unwrap();
    assert_eq!(proj_a.sessions, 3);
    assert_eq!(proj_a.name, "a");

    assert!(
        !r.heatmap.is_empty(),
        "heatmap should contain at least one cell"
    );
    for cell in &r.heatmap {
        assert!(cell.weekday <= 6);
        assert!(cell.hour <= 23);
    }
    // 4 个 session 跨两天 → daily 应有 2 个桶
    assert_eq!(r.daily.len(), 2);
    let today_total: i64 = r.daily.iter().map(|d| d.sessions).sum();
    assert_eq!(today_total, 4);
}

#[test]
fn test_workload_report_excludes_old_sessions() {
    let db = Db::open_in_memory().unwrap();
    // 一个 100 天前的 session 不应该出现在 30 天窗口里
    let old = (chrono::Local::now() - chrono::Duration::days(100))
        .format("%Y-%m-%dT%H:%M:%S+08:00")
        .to_string();
    ws_seed_session(&db, "old", "cursor", Some("/old"), &old, 1);
    let r = db.workload_report(30).unwrap();
    assert_eq!(r.overall.sessions, 0);
}
