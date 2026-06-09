//! sessions_needing_summary 的方案 A / B 闸门 + intent + session detail。

use crate::storage::db::{self, Db};

/// 方案 A —— 过期检测：摘要生成后，session 又涨了新消息，应被重新纳入候选。
///
/// 这是我们要修复的核心 bug：原查询是 `WHERE sm.id IS NULL`，
/// 一旦摘要存在就永远不再列出，导致 t=5s 后到来的新消息永远进不了 L2 摘要。
#[test]
fn test_sessions_needing_summary_detects_stale_summary() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h("msg1"))
        .unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h("msg2"))
        .unwrap();

    db.upsert_summary(db::SummaryUpsert {
        session_id: "s1",
        level: "L2_session",
        title: Some("title"),
        summary: "summary",
        topics: &[],
        decisions: &[],
        message_count_at_creation: 2,
    })
    .unwrap();

    // 此刻 message_count(=2) == message_count_at_creation(=2)：未过期。
    assert!(
        db.sessions_needing_summary(10, 0).unwrap().is_empty(),
        "刚摘要完的 session 不应再次被纳入候选"
    );

    db.insert_message("m3", "s1", "user", "msg3", None, 2, &h("msg3"))
        .unwrap();

    let needing = db.sessions_needing_summary(10, 0).unwrap();
    assert_eq!(
        needing,
        vec!["s1".to_string()],
        "新消息写入后，旧摘要应被视为过期、重新进入候选（方案 A 过期检测）"
    );
}

/// 方案 B —— 会话冷却：updated_at 距今不到冷却时间的会话不应被纳入候选。
/// 用 cool_down_secs = 1 hour 确保确定性，避免依赖测试机时钟。
#[test]
fn test_sessions_needing_summary_respects_cooldown() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h("msg1"))
        .unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h("msg2"))
        .unwrap();

    assert!(
        db.sessions_needing_summary(10, 3600).unwrap().is_empty(),
        "updated_at 在冷却窗口内的 session 不应被列出（方案 B 冷却）"
    );

    assert_eq!(
        db.sessions_needing_summary(10, 0).unwrap(),
        vec!["s1".to_string()],
        "cool_down_secs=0 时应等价于「无冷却」"
    );

    let old = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = 's1'",
            rusqlite::params![old],
        )
        .unwrap();
    }
    assert_eq!(
        db.sessions_needing_summary(10, 3600).unwrap(),
        vec!["s1".to_string()],
        "updated_at 老于 cool_down_secs 的 session 应通过冷却闸门"
    );
}

/// 综合：冷却 + 过期检测都要满足。
#[test]
fn test_sessions_needing_summary_cooldown_gates_stale_too() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h("msg1"))
        .unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h("msg2"))
        .unwrap();
    db.upsert_summary(db::SummaryUpsert {
        session_id: "s1",
        level: "L2_session",
        title: Some("t"),
        summary: "s",
        topics: &[],
        decisions: &[],
        message_count_at_creation: 2,
    })
    .unwrap();

    db.insert_message("m3", "s1", "user", "msg3", None, 2, &h("msg3"))
        .unwrap();

    assert!(
        db.sessions_needing_summary(10, 3600).unwrap().is_empty(),
        "即便摘要已过期，冷却中也不应立刻重摘要 —— 避免高频抖动"
    );

    let old = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    {
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = 's1'",
            rusqlite::params![old],
        )
        .unwrap();
    }
    assert_eq!(
        db.sessions_needing_summary(10, 3600).unwrap(),
        vec!["s1".to_string()],
        "冷却 + 过期同时满足时，应纳入候选 —— 方案 A+B 组合命中"
    );
}

/// 只有 1 条消息的 session 永远不应被列出（与 summarize_session_by_id 的
/// `messages.len() >= 2` 守门保持一致）。
#[test]
fn test_sessions_needing_summary_skips_short_sessions() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    let h = blake3::hash(b"x").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "x", None, 0, &h)
        .unwrap();
    assert!(
        db.sessions_needing_summary(10, 0).unwrap().is_empty(),
        "message_count < 2 的 session 不该被摘要候选列出"
    );
}

/// v7 新增：sessions.intent 列与 update_session_intent。
#[test]
fn test_update_session_intent_roundtrip() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl", 0, 0)
        .unwrap();
    let h = blake3::hash(b"hello").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "hello", None, 0, &h)
        .unwrap();
    db.insert_message("m2", "s1", "assistant", "world", None, 1, &h)
        .unwrap();

    let row = &db.list_sessions_paged(10, 0).unwrap()[0];
    assert!(row.intent.is_none(), "新建 session 的 intent 默认为 NULL");

    db.update_session_intent("s1", Some("修复登录")).unwrap();
    let row = &db.list_sessions_paged(10, 0).unwrap()[0];
    assert_eq!(row.intent.as_deref(), Some("修复登录"));

    db.update_session_intent("s1", None).unwrap();
    let row = &db.list_sessions_paged(10, 0).unwrap()[0];
    assert!(row.intent.is_none(), "update_session_intent(None) 应清空");
}

/// SessionDetail.intent 也要从 sessions.intent 读出。
#[test]
fn test_get_session_detail_includes_intent() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "cursor", Some("/proj"), "/f.jsonl", 0, 0)
        .unwrap();
    db.update_session_intent("s1", Some("调研 monthly report"))
        .unwrap();
    let detail = db.get_session_detail("s1").unwrap().unwrap();
    assert_eq!(detail.intent.as_deref(), Some("调研 monthly report"));
}

#[test]
fn test_list_sessions_filters_only_stale_empty_sessions() {
    let db = Db::open_in_memory().unwrap();
    let old = (chrono::Utc::now() - chrono::Duration::days(2)).to_rfc3339();
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO sessions (id, source, file_path, created_at, updated_at, message_count)
         VALUES ('old_empty', 'cursor', '/old.jsonl', ?1, ?1, 0)",
        rusqlite::params![old],
    )
    .unwrap();
    drop(conn);

    db.insert_session("recent_empty", "cursor", None, "/recent.jsonl", 0, 0)
        .unwrap();
    db.insert_session("old_with_messages", "cursor", None, "/messages.jsonl", 0, 0)
        .unwrap();
    let hash = blake3::hash(b"kept").to_hex().to_string();
    db.insert_message("m1", "old_with_messages", "user", "kept", None, 0, &hash)
        .unwrap();

    let ids: Vec<String> = db
        .list_sessions_paged(10, 0)
        .unwrap()
        .into_iter()
        .map(|row| row.id)
        .collect();

    assert!(!ids.contains(&"old_empty".to_string()));
    assert!(ids.contains(&"recent_empty".to_string()));
    assert!(ids.contains(&"old_with_messages".to_string()));
}

/// 回归：`messages.timestamp` 为 NULL（cursor / continue_dev adapter）时
/// `get_session_detail` 必须用 `sessions.updated_at` 退化填充，前端 UI 才能
/// 始终渲染消息时间戳。
#[test]
fn test_get_session_detail_falls_back_to_session_updated_at_for_null_message_ts() {
    let db = Db::open_in_memory().unwrap();
    let session_updated_at = "2026-06-01 10:00:00";

    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at, message_count)
         VALUES ('s1', 'cursor', '/p', '/f.jsonl', ?1, ?1, 0)",
        rusqlite::params![session_updated_at],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
         VALUES ('m1', 's1', 'user', 'hello', NULL, 0, 'h1'),
                ('m2', 's1', 'assistant', 'hi', NULL, 1, 'h2')",
        [],
    )
    .unwrap();
    drop(conn);

    let detail = db.get_session_detail("s1").unwrap().unwrap();
    assert_eq!(detail.messages.len(), 2);
    for m in &detail.messages {
        assert_eq!(
            m.timestamp.as_deref(),
            Some(session_updated_at),
            "message {} should fall back to session.updated_at",
            m.id
        );
    }
}

/// 反向回归：messages.timestamp 有真实值时不能被 COALESCE 替换掉。
#[test]
fn test_get_session_detail_keeps_real_message_timestamp() {
    let db = Db::open_in_memory().unwrap();
    let session_updated_at = "2026-06-01 10:00:00";
    let m1_ts = "2026-06-01 09:01:23";
    let m2_ts = "2026-06-01 09:05:00";

    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at, message_count)
         VALUES ('s1', 'claude_code', '/p', '/f.jsonl', ?1, ?1, 0)",
        rusqlite::params![session_updated_at],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
         VALUES ('m1', 's1', 'user', 'q', ?1, 0, 'h1'),
                ('m2', 's1', 'assistant', 'a', ?2, 1, 'h2')",
        rusqlite::params![m1_ts, m2_ts],
    )
    .unwrap();
    drop(conn);

    let detail = db.get_session_detail("s1").unwrap().unwrap();
    assert_eq!(detail.messages[0].timestamp.as_deref(), Some(m1_ts));
    assert_eq!(detail.messages[1].timestamp.as_deref(), Some(m2_ts));
}
