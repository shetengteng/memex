//! 基础过滤：空 filter（语义与 plain paged 等价）、adapter 多选、
//! project 末段名匹配、message_count 排序。

use super::{seed_filtered_session, SessionSeed};
use crate::storage::db::sessions::SessionListFilter;
use crate::storage::db::Db;

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
fn filtered_paged_projects_exact_path_does_not_cross_match_same_endsegment() {
    // 关键回归：facet 行的语义 = 单个 (project_path, count) 元组。
    // 勾选 `/A/src` 不应该捎带 `/B/src`，否则就回到了"末段串扰"老 bug。
    // 旧实现用 LIKE '%/src' 一并命中所有 *.src 路径，导致 facet 计数与
    // 勾选后列表数差异巨大；现在改成 IN (?) 精确匹配，这里固化语义。
    let db = Db::open_in_memory().unwrap();
    for (id, project_path) in [
        ("a_src", "/repo/A/src"),
        ("b_src", "/repo/B/src"),
        ("a_root", "/repo/A"),
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
                projects: Some(vec!["/repo/A/src".into()]),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["a_src"]);
}

#[test]
fn filtered_paged_projects_exact_path_rejects_prefix_collisions() {
    // 防止误把"完整 path"当成"前缀"：选 `/repo/memex` 不能匹配 `/repo/memex-clone`。
    // LIKE '/repo/memex%' 会回到老 bug；IN (?) 走值相等，天然安全。
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
                projects: Some(vec!["/Users/me/repo/memex".into()]),
                ..Default::default()
            },
            10,
            0,
        )
        .unwrap();
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, vec!["real_memex"]);
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
