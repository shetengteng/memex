//! 维度特化：time 边界、L2 summary 完成度、query 跨字段全文匹配、
//! adapter + project + time + query 复合命中。

use super::{SessionSeed, seed_filtered_session};
use crate::storage::db::sessions::SessionListFilter;
use crate::storage::db::{self, Db};

#[test]
fn filtered_paged_time_today_returns_only_today_updated() {
    let db = Db::open_in_memory().unwrap();
    let today = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let last_week = (chrono::Utc::now() - chrono::Duration::days(8))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    for (id, ts) in [("today_s", today.as_str()), ("old_s", last_week.as_str())] {
        seed_filtered_session(
            &db,
            SessionSeed {
                id,
                source: "cursor",
                project_path: None,
                title: None,
                created_at: ts,
                updated_at: ts,
                message_count: 1,
            },
        );
    }

    let rows = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                time: Some("today".into()),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&"today_s"));
    assert!(!ids.contains(&"old_s"));
}

#[test]
fn filtered_paged_summary_done_and_pending_partition_the_set() {
    let db = Db::open_in_memory().unwrap();
    for id in ["with_l2", "no_l2"] {
        seed_filtered_session(
            &db,
            SessionSeed {
                id,
                source: "cursor",
                project_path: None,
                title: None,
                created_at: "2026-06-01 10:00:00",
                updated_at: "2026-06-01 10:00:00",
                message_count: 2,
            },
        );
    }
    db.upsert_summary(db::SummaryUpsert {
        session_id: "with_l2",
        level: "L2_session",
        title: Some("title"),
        summary: "summary",
        topics: &[],
        decisions: &[],
        message_count_at_creation: 2,
    })
    .unwrap();

    let done = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                summary: Some("done".into()),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].id, "with_l2");

    let pending = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                summary: Some("pending".into()),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "no_l2");
}

#[test]
fn filtered_paged_query_matches_title_or_intent_or_summary() {
    let db = Db::open_in_memory().unwrap();
    for (id, title) in [
        ("by_title", "redis cache"),
        ("by_intent", "unrelated title"),
        ("unrelated", "sqlite migration"),
    ] {
        seed_filtered_session(
            &db,
            SessionSeed {
                id,
                source: "cursor",
                project_path: None,
                title: Some(title),
                created_at: "2026-06-01 10:00:00",
                updated_at: "2026-06-01 10:00:00",
                message_count: 2,
            },
        );
    }
    db.update_session_intent("by_intent", Some("debug redis connection issue"))
        .unwrap();

    let rows = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                query: Some("redis".into()),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&"by_title"));
    assert!(ids.contains(&"by_intent"));
    assert!(!ids.contains(&"unrelated"));
}

#[test]
fn filtered_paged_composite_adapter_project_time_query() {
    let db = Db::open_in_memory().unwrap();
    let today = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let old = (chrono::Utc::now() - chrono::Duration::days(40))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    let cases: &[(&str, &str, &str, &str, &str)] = &[
        ("match", "cursor", "/repo/memex", "redis bug", &today),
        (
            "wrong_adapter",
            "claude_code",
            "/repo/memex",
            "redis bug",
            &today,
        ),
        (
            "wrong_project",
            "cursor",
            "/repo/other",
            "redis bug",
            &today,
        ),
        ("wrong_time", "cursor", "/repo/memex", "redis bug", &old),
        (
            "wrong_query",
            "cursor",
            "/repo/memex",
            "sqlite topic",
            &today,
        ),
    ];
    for (id, source, project_path, title, ts) in cases {
        seed_filtered_session(
            &db,
            SessionSeed {
                id,
                source,
                project_path: Some(project_path),
                title: Some(title),
                created_at: ts,
                updated_at: ts,
                message_count: 3,
            },
        );
    }

    let rows = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                adapters: Some(vec!["cursor".into()]),
                projects: Some(vec!["/repo/memex".into()]),
                time: Some("7d".into()),
                query: Some("redis".into()),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["match"]);
}
