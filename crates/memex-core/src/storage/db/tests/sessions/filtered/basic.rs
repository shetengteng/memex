//! 基础过滤：空 filter（语义与 plain paged 等价）、adapter 多选、
//! project 末段名匹配、message_count 排序。

use super::{SessionSeed, seed_filtered_session};
use crate::storage::db::Db;
use crate::storage::db::sessions::SessionListFilter;

#[test]
fn filtered_paged_empty_filter_matches_plain_paged() {
    let db = Db::open_in_memory().unwrap();
    seed_filtered_session(
        &db,
        SessionSeed {
            id: "s1",
            source: "cursor",
            project_path: Some("/workspace/a"),
            title: Some("alpha"),
            created_at: "2026-06-01 10:00:00",
            updated_at: "2026-06-01 10:05:00",
            message_count: 3,
        },
    );
    seed_filtered_session(
        &db,
        SessionSeed {
            id: "s2",
            source: "claude_code",
            project_path: Some("/workspace/b"),
            title: Some("beta"),
            created_at: "2026-06-02 10:00:00",
            updated_at: "2026-06-02 10:05:00",
            message_count: 5,
        },
    );

    let plain = db.list_sessions_paged(10, 0).unwrap();
    let filtered = db
        .list_sessions_filtered_paged(&SessionListFilter::default(), 10, 0)
        .unwrap();

    let plain_ids: Vec<&str> = plain.iter().map(|r| r.id.as_str()).collect();
    let filtered_ids: Vec<&str> = filtered.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(filtered_ids, plain_ids);
}

#[test]
fn filtered_paged_adapters_multi_select() {
    let db = Db::open_in_memory().unwrap();
    for (id, source) in [
        ("s1", "cursor"),
        ("s2", "claude_code"),
        ("s3", "codex"),
        ("s4", "opencode"),
    ] {
        seed_filtered_session(
            &db,
            SessionSeed {
                id,
                source,
                project_path: None,
                title: None,
                created_at: "2026-06-01 10:00:00",
                updated_at: "2026-06-01 10:00:00",
                message_count: 2,
            },
        );
    }

    let only_codex = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                adapters: Some(vec!["codex".into()]),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    assert_eq!(only_codex.len(), 1);
    assert_eq!(only_codex[0].id, "s3");

    let two = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                adapters: Some(vec!["cursor".into(), "codex".into()]),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    let ids: Vec<&str> = two.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&"s1"));
    assert!(ids.contains(&"s3"));
    assert!(!ids.contains(&"s2"));
    assert!(!ids.contains(&"s4"));
}

#[test]
fn filtered_paged_projects_endsegment_does_not_match_similarly_named_paths() {
    let db = Db::open_in_memory().unwrap();
    for (id, project_path) in [
        ("real_memex", "/Users/me/repo/memex"),
        ("memex_clone", "/Users/me/repo/memex-clone"),
        ("memex_nested", "/Users/me/repo/foo/memex"),
    ] {
        seed_filtered_session(
            &db,
            SessionSeed {
                id,
                source: "cursor",
                project_path: Some(project_path),
                title: None,
                created_at: "2026-06-01 10:00:00",
                updated_at: "2026-06-01 10:00:00",
                message_count: 1,
            },
        );
    }

    let rows = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                projects: Some(vec!["memex".into()]),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert!(ids.contains(&"real_memex"));
    assert!(ids.contains(&"memex_nested"));
    assert!(!ids.contains(&"memex_clone"));
}

#[test]
fn filtered_paged_sort_messages_orders_by_message_count_desc() {
    let db = Db::open_in_memory().unwrap();
    for (id, message_count) in [("few", 2), ("many", 50), ("mid", 20)] {
        seed_filtered_session(
            &db,
            SessionSeed {
                id,
                source: "cursor",
                project_path: None,
                title: None,
                created_at: "2026-06-01 10:00:00",
                updated_at: "2026-06-01 10:00:00",
                message_count,
            },
        );
    }

    let rows = db
        .list_sessions_filtered_paged(
            &SessionListFilter {
                sort: Some("messages".into()),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["many", "mid", "few"]);
}
