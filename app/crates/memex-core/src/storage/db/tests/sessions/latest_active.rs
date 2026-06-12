//! `Db::latest_active_project()` 行为契约。
//!
//! 这条查询专门服务 `memex context` 的 IDE-cwd fallback：当 Cursor /
//! Claude Code 的 sessionStart hook 把 `$PWD` 指向 `~/.cursor` /
//! `~/.claude`，三级 project 匹配全部失败时，本方法返回「用户最近活跃
//! 的项目路径」当兜底，避免给 AI 的工作记忆 banner 留空。

use rusqlite::params;

use crate::storage::db::Db;

fn insert_session_at(
    db: &Db,
    id: &str,
    project_path: Option<&str>,
    updated_at: &str,
    message_count: i64,
) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at, message_count)
         VALUES (?1, 'cursor', ?2, '/tmp/f.jsonl', ?3, ?3, ?4)",
        params![id, project_path, updated_at, message_count],
    )
    .unwrap();
}

#[test]
fn returns_none_when_no_sessions() {
    let db = Db::open_in_memory().unwrap();
    assert_eq!(db.latest_active_project().unwrap(), None);
}

#[test]
fn returns_none_when_all_sessions_lack_project_path() {
    let db = Db::open_in_memory().unwrap();
    insert_session_at(&db, "s1", None, "2026-06-09 10:00:00", 5);
    insert_session_at(&db, "s2", Some(""), "2026-06-10 10:00:00", 3);
    assert_eq!(db.latest_active_project().unwrap(), None);
}

#[test]
fn returns_project_of_most_recently_updated_session() {
    let db = Db::open_in_memory().unwrap();
    insert_session_at(&db, "s1", Some("/p/old"), "2026-06-01 10:00:00", 5);
    insert_session_at(&db, "s2", Some("/p/middle"), "2026-06-05 10:00:00", 5);
    insert_session_at(&db, "s3", Some("/p/latest"), "2026-06-10 10:00:00", 5);
    assert_eq!(
        db.latest_active_project().unwrap().as_deref(),
        Some("/p/latest")
    );
}

#[test]
fn skips_orphan_zero_message_sessions_older_than_one_day() {
    // 旧的 message_count=0 session（与 list_sessions_* 系列一致的过滤规则）
    // 不应顶到最前面，避免「未补完的扫描中会话」抢断真实活跃项目。
    let db = Db::open_in_memory().unwrap();
    let two_days_ago = (chrono::Utc::now() - chrono::Duration::days(2))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let three_days_ago = (chrono::Utc::now() - chrono::Duration::days(3))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();

    // 最新但是孤儿（应被过滤）
    insert_session_at(&db, "s_orphan", Some("/p/orphan"), &two_days_ago, 0);
    // 略旧但是真实有消息（应胜出）
    insert_session_at(&db, "s_real", Some("/p/real"), &three_days_ago, 7);

    assert_eq!(
        db.latest_active_project().unwrap().as_deref(),
        Some("/p/real"),
        "孤儿（msg=0 且超 1 天）应被过滤，让真实活跃项目胜出"
    );
}

#[test]
fn preserves_recent_zero_message_sessions_within_one_day() {
    // 反向回归：刚扫到（< 1 天）但还没补到 message 的 session **不** 该被过滤，
    // 否则用户刚切到的新项目第一时间会被 fallback 忽略。
    let db = Db::open_in_memory().unwrap();
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    insert_session_at(&db, "s_new", Some("/p/newly_scanned"), &now, 0);
    assert_eq!(
        db.latest_active_project().unwrap().as_deref(),
        Some("/p/newly_scanned"),
    );
}
