use super::Db;
use crate::storage::models::{Chunk, ChunkMetadata, ChunkType, SourceState};

#[test]
fn test_schema_init() {
    let db = Db::open_in_memory().unwrap();
    assert_eq!(db.session_count().unwrap(), 0);
}

#[test]
fn test_insert_and_dedup() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl", 0, 0)
        .unwrap();
    let hash = blake3::hash(b"hello").to_hex().to_string();
    assert!(
        db.insert_message("m1", "s1", "user", "hello", None, 0, &hash)
            .unwrap()
    );
    assert!(
        !db.insert_message("m2", "s1", "user", "hello", None, 0, &hash)
            .unwrap()
    );
    assert_eq!(db.message_count().unwrap(), 1);
}

/// 回归测试：insert_message **不应该**用 `Utc::now()` 覆盖
/// `sessions.updated_at`。否则把 ingest_adapter 阶段写入的真实 mtime
/// （cursor `composer.last_updated_at` / claude_code 文件 mtime 等）
/// 全部刷成「今天」，会话列表里所有日期都被推到当天，掩盖真实活动时间。
#[test]
fn test_insert_message_does_not_overwrite_session_updated_at() {
    use rusqlite::params;

    let db = Db::open_in_memory().unwrap();
    // 模拟 cursor 上报的真实 mtime：2025-06-11（一年前）。
    let real_mtime_secs: u64 = 1_749_628_448; // 2025-06-11T07:54:08Z
    db.insert_session("s1", "cursor", None, "/state.vscdb#composer=s1", real_mtime_secs, real_mtime_secs)
        .unwrap();

    let updated_before: String = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT updated_at FROM sessions WHERE id = ?1",
            params!["s1"],
            |row| row.get(0),
        ).unwrap()
    };
    assert!(
        updated_before.starts_with("2025-06-11"),
        "insert_session 必须用真实 mtime 写入 updated_at，实际：{}",
        updated_before
    );

    // 写入若干新消息，模拟 ingest_adapter 后续把消息批量写入。
    let h1 = blake3::hash(b"msg1").to_hex().to_string();
    let h2 = blake3::hash(b"msg2").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h1).unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h2).unwrap();

    let updated_after: String = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT updated_at FROM sessions WHERE id = ?1",
            params!["s1"],
            |row| row.get(0),
        ).unwrap()
    };
    assert_eq!(
        updated_before, updated_after,
        "insert_message 不能覆盖 sessions.updated_at —— 这是 cursor 历史会话\n\
         updated_at 全部变成今天的根因。"
    );

    // message_count 仍应正常自增。
    let count: i64 = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT message_count FROM sessions WHERE id = ?1",
            params!["s1"],
            |row| row.get(0),
        ).unwrap()
    };
    assert_eq!(count, 2, "insert_message 仍应维护 message_count 自增");
}

#[test]
fn test_fts_search() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0).unwrap();
    let hash = blake3::hash(b"redis pipeline").to_hex().to_string();
    db.insert_message("m1", "s1", "assistant", "redis pipeline", None, 0, &hash)
        .unwrap();
    let chunk = Chunk {
        id: None,
        message_id: "m1".into(),
        session_id: "s1".into(),
        chunk_type: ChunkType::Text,
        content: "redis pipeline".into(),
        redacted_content: None,
        position: 0,
        token_count: 3,
        metadata: ChunkMetadata::default(),
    };
    db.insert_chunk(&chunk).unwrap();
    let results = db.fts_search("redis", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].adapter.as_deref(), Some("claude_code"));
}

#[test]
fn test_source_offset() {
    let db = Db::open_in_memory().unwrap();
    let state = SourceState {
        adapter: "test".into(),
        file_path: "/test.jsonl".into(),
        last_offset: 1024,
        last_mtime: 0,
        last_scan: chrono::Utc::now(),
    };
    db.upsert_source(&state).unwrap();
    assert_eq!(db.get_source_offset("/test.jsonl").unwrap(), 1024);
}

#[test]
fn test_kv_roundtrip() {
    let db = Db::open_in_memory().unwrap();
    assert!(db.kv_get("missing").unwrap().is_none());
    db.kv_set("k", "v1").unwrap();
    assert_eq!(db.kv_get("k").unwrap().as_deref(), Some("v1"));
    db.kv_set("k", "v2").unwrap();
    assert_eq!(db.kv_get("k").unwrap().as_deref(), Some("v2"));
}

#[test]
fn test_summary_upsert_and_get() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0).unwrap();
    db.upsert_summary(
        "s1", "L2_session", Some("Fix auth bug"),
        "Fixed JWT parsing issue.", &["auth".into()], &["use RS256".into()],
        10,
    ).unwrap();

    let summary = db.get_summary("s1", "L2_session").unwrap().unwrap();
    assert_eq!(summary.title.as_deref(), Some("Fix auth bug"));
    assert_eq!(summary.topics, vec!["auth"]);
    assert_eq!(summary.decisions, vec!["use RS256"]);

    db.upsert_summary(
        "s1", "L2_session", Some("Updated title"),
        "Updated summary.", &["auth".into(), "jwt".into()], &[],
        20,
    ).unwrap();
    let updated = db.get_summary("s1", "L2_session").unwrap().unwrap();
    assert_eq!(updated.title.as_deref(), Some("Updated title"));
    assert_eq!(updated.topics.len(), 2);
}

#[test]
fn test_summary_not_found() {
    let db = Db::open_in_memory().unwrap();
    assert!(db.get_summary("nonexist", "L2_session").unwrap().is_none());
}

#[test]
fn test_chunk_summary_update() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0).unwrap();
    let hash = blake3::hash(b"content").to_hex().to_string();
    db.insert_message("m1", "s1", "assistant", "content", None, 0, &hash).unwrap();
    let chunk = Chunk {
        id: None,
        message_id: "m1".into(),
        session_id: "s1".into(),
        chunk_type: ChunkType::Text,
        content: "This is a long piece of content about implementing a Redis caching layer.".into(),
        redacted_content: None,
        position: 0,
        token_count: 50,
        metadata: ChunkMetadata::default(),
    };
    let chunk_id = db.insert_chunk(&chunk).unwrap();

    let unsummarized = db.chunks_without_summary(10, 10).unwrap();
    assert_eq!(unsummarized.len(), 1);
    assert_eq!(unsummarized[0].0, chunk_id);

    db.update_chunk_summary(chunk_id, "Implemented Redis caching.").unwrap();

    let after = db.chunks_without_summary(10, 10).unwrap();
    assert!(after.is_empty());
}

#[test]
fn test_chunks_without_summary_respects_min_tokens() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "cursor", None, "/f.jsonl", 0, 0).unwrap();
    let hash = blake3::hash(b"tiny").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "tiny", None, 0, &hash).unwrap();
    let small_chunk = Chunk {
        id: None,
        message_id: "m1".into(),
        session_id: "s1".into(),
        chunk_type: ChunkType::Text,
        content: "small".into(),
        redacted_content: None,
        position: 0,
        token_count: 2,
        metadata: ChunkMetadata::default(),
    };
    db.insert_chunk(&small_chunk).unwrap();

    let results = db.chunks_without_summary(50, 10).unwrap();
    assert!(results.is_empty(), "chunks below min_token_count should be excluded");
}

#[test]
fn test_aggregate_summary_upsert_and_get() {
    let db = Db::open_in_memory().unwrap();
    db.upsert_aggregate_summary(
        "project", "/my/project",
        Some("My Project"), "Project-level summary.",
        &["rust".into()], &["use FTS5".into()], 3,
    ).unwrap();

    let s = db.get_aggregate_summary("project", "/my/project").unwrap().unwrap();
    assert_eq!(s.title.as_deref(), Some("My Project"));
    assert_eq!(s.session_count, 3);

    db.upsert_aggregate_summary(
        "project", "/my/project",
        Some("Updated Project"), "Updated summary.",
        &["rust".into(), "search".into()], &[], 5,
    ).unwrap();
    let updated = db.get_aggregate_summary("project", "/my/project").unwrap().unwrap();
    assert_eq!(updated.title.as_deref(), Some("Updated Project"));
    assert_eq!(updated.session_count, 5);
}

#[test]
fn test_aggregate_summary_not_found() {
    let db = Db::open_in_memory().unwrap();
    assert!(db.get_aggregate_summary("project", "nonexist").unwrap().is_none());
}

/// 方案 A —— 过期检测：摘要生成后，session 又涨了新消息，应被重新纳入候选。
///
/// 这是我们要修复的核心 bug：原查询是 `WHERE sm.id IS NULL`，
/// 一旦摘要存在就永远不再列出，导致 t=5s 后到来的新消息永远进不了 L2 摘要。
#[test]
fn test_sessions_needing_summary_detects_stale_summary() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0).unwrap();
    let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h("msg1")).unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h("msg2")).unwrap();

    db.upsert_summary(
        "s1", "L2_session", Some("title"), "summary",
        &[], &[], /* message_count_at_creation = */ 2,
    ).unwrap();

    // 此刻 message_count(=2) == message_count_at_creation(=2)：未过期。
    assert!(
        db.sessions_needing_summary(10, 0).unwrap().is_empty(),
        "刚摘要完的 session 不应再次被纳入候选"
    );

    // 模拟 t=5s 又来了新消息。
    db.insert_message("m3", "s1", "user", "msg3", None, 2, &h("msg3")).unwrap();

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
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0).unwrap();
    let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h("msg1")).unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h("msg2")).unwrap();
    // 此时 sessions.updated_at = now()，距今 < 1 小时。

    // cool_down_secs = 3600（1 小时）：session 还在冷却中，应被排除。
    assert!(
        db.sessions_needing_summary(10, 3600).unwrap().is_empty(),
        "updated_at 在冷却窗口内的 session 不应被列出（方案 B 冷却）"
    );

    // cool_down_secs = 0：跳过冷却，立刻可见。
    assert_eq!(
        db.sessions_needing_summary(10, 0).unwrap(),
        vec!["s1".to_string()],
        "cool_down_secs=0 时应等价于「无冷却」"
    );

    // 把 updated_at 拨回 2 小时前 → 通过冷却（方案 B 命中）。
    let old = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = 's1'",
            rusqlite::params![old],
        ).unwrap();
    }
    assert_eq!(
        db.sessions_needing_summary(10, 3600).unwrap(),
        vec!["s1".to_string()],
        "updated_at 老于 cool_down_secs 的 session 应通过冷却闸门"
    );
}

/// 综合：冷却 + 过期检测都要满足。
/// session 已有摘要、且最近又有新消息 → 一边过期（应进）一边在冷却窗口（不该进）→ 不进；
/// 等会话冷却下来后再进。
#[test]
fn test_sessions_needing_summary_cooldown_gates_stale_too() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0).unwrap();
    let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h("msg1")).unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h("msg2")).unwrap();
    db.upsert_summary("s1", "L2_session", Some("t"), "s", &[], &[], 2).unwrap();

    // 新消息进来 → 过期；同时 updated_at = now() → 在冷却窗口。
    db.insert_message("m3", "s1", "user", "msg3", None, 2, &h("msg3")).unwrap();

    assert!(
        db.sessions_needing_summary(10, 3600).unwrap().is_empty(),
        "即便摘要已过期，冷却中也不应立刻重摘要 —— 避免高频抖动"
    );

    // 把 updated_at 拨老 → 同时通过冷却 + 过期检测。
    let old = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET updated_at = ?1 WHERE id = 's1'",
            rusqlite::params![old],
        ).unwrap();
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
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0).unwrap();
    let h = blake3::hash(b"x").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "x", None, 0, &h).unwrap();
    assert!(
        db.sessions_needing_summary(10, 0).unwrap().is_empty(),
        "message_count < 2 的 session 不该被摘要候选列出"
    );
}
