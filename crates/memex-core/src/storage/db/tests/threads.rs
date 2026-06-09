//! L5「主题线索」表与 upsert/delete/list/detail 行为回归。

use crate::storage::db::{Db, ThreadDraft};

fn seed_sessions(db: &Db, ids: &[&str]) {
    for id in ids {
        db.insert_session(id, "cursor", Some("/proj"), "/f", 0, 0)
            .unwrap();
    }
}

#[test]
fn test_upsert_thread_creates_then_updates_idempotent() {
    let db = Db::open_in_memory().unwrap();
    seed_sessions(&db, &["s1", "s2", "s3"]);

    let id1 = db
        .upsert_thread_with_sessions(&ThreadDraft {
            name: "桌面化".into(),
            summary: "迁移到 Tauri 桌面应用".into(),
            session_ids: vec!["s1".into(), "s2".into()],
        })
        .unwrap();

    let detail = db.get_thread_detail(id1).unwrap().unwrap();
    assert_eq!(detail.thread.name, "桌面化");
    assert_eq!(detail.thread.session_count, 2);
    assert_eq!(detail.sessions.len(), 2);

    // 第二次 upsert 同名 thread——session 集合改成 (s2, s3)
    let id2 = db
        .upsert_thread_with_sessions(&ThreadDraft {
            name: "桌面化".into(),
            summary: "v2".into(),
            session_ids: vec!["s2".into(), "s3".into()],
        })
        .unwrap();
    assert_eq!(id1, id2, "同名 thread upsert 必须返回同一个 id");

    let detail = db.get_thread_detail(id2).unwrap().unwrap();
    assert_eq!(detail.thread.summary, "v2");
    assert_eq!(detail.thread.session_count, 2);
    let ids: Vec<&str> = detail.sessions.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"s2"));
    assert!(ids.contains(&"s3"));
    assert!(!ids.contains(&"s1"), "旧 link (s1) 应该被清掉");
}

#[test]
fn test_upsert_thread_skips_missing_session_ids() {
    let db = Db::open_in_memory().unwrap();
    seed_sessions(&db, &["s1"]);

    let id = db
        .upsert_thread_with_sessions(&ThreadDraft {
            name: "x".into(),
            summary: String::new(),
            session_ids: vec!["s1".into(), "s2_nonexist".into(), "s3_nonexist".into()],
        })
        .unwrap();
    let detail = db.get_thread_detail(id).unwrap().unwrap();
    assert_eq!(detail.thread.session_count, 1);
    assert_eq!(detail.sessions.len(), 1);
    assert_eq!(detail.sessions[0].id, "s1");
}

#[test]
fn test_list_threads_orders_by_updated_at_desc() {
    let db = Db::open_in_memory().unwrap();
    seed_sessions(&db, &["s1"]);

    db.upsert_thread_with_sessions(&ThreadDraft {
        name: "a".into(),
        summary: String::new(),
        session_ids: vec!["s1".into()],
    })
    .unwrap();
    // 第二个 thread 在第一个之后插入——updated_at 更新
    std::thread::sleep(std::time::Duration::from_millis(20));
    db.upsert_thread_with_sessions(&ThreadDraft {
        name: "b".into(),
        summary: String::new(),
        session_ids: vec!["s1".into()],
    })
    .unwrap();

    let rows = db.list_threads_paged(10, 0).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "b");
    assert_eq!(rows[1].name, "a");
}

#[test]
fn test_delete_thread_cascades_thread_sessions() {
    let db = Db::open_in_memory().unwrap();
    seed_sessions(&db, &["s1"]);

    let id = db
        .upsert_thread_with_sessions(&ThreadDraft {
            name: "x".into(),
            summary: String::new(),
            session_ids: vec!["s1".into()],
        })
        .unwrap();

    db.delete_thread(id).unwrap();

    assert!(db.get_thread_detail(id).unwrap().is_none());
    let conn = db.conn.lock();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM thread_sessions WHERE thread_id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

/// list_threads_paged 必须一次返回卡片视图需要的聚合字段：
/// first_session_at / last_session_at / projects / adapters。
#[test]
fn test_list_threads_returns_aggregate_fields() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s_a", "cursor", Some("/proj-a"), "/f", 0, 0)
        .unwrap();
    db.insert_session("s_b", "claude_code", Some("/proj-b"), "/f", 0, 0)
        .unwrap();

    db.upsert_thread_with_sessions(&ThreadDraft {
        name: "mixed".into(),
        summary: String::new(),
        session_ids: vec!["s_a".into(), "s_b".into()],
    })
    .unwrap();

    let rows = db.list_threads_paged(10, 0).unwrap();
    assert_eq!(rows.len(), 1);
    let r = &rows[0];

    assert!(r.first_session_at.is_some(), "first_session_at 必须非空");
    assert!(r.last_session_at.is_some(), "last_session_at 必须非空");

    assert_eq!(r.projects.len(), 2);
    assert!(r.projects.contains(&"/proj-a".to_string()));
    assert!(r.projects.contains(&"/proj-b".to_string()));
    assert_eq!(r.adapters.len(), 2);
    assert!(r.adapters.contains(&"claude_code".to_string()));
    assert!(r.adapters.contains(&"cursor".to_string()));

    let detail = db.get_thread_detail(r.id).unwrap().unwrap();
    assert_eq!(detail.thread.projects.len(), 2);
    assert_eq!(detail.thread.adapters.len(), 2);
}
