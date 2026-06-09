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
    db.insert_session(
        "s1",
        "cursor",
        None,
        "/state.vscdb#composer=s1",
        real_mtime_secs,
        real_mtime_secs,
    )
    .unwrap();

    let updated_before: String = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT updated_at FROM sessions WHERE id = ?1",
            params!["s1"],
            |row| row.get(0),
        )
        .unwrap()
    };
    assert!(
        updated_before.starts_with("2025-06-11"),
        "insert_session 必须用真实 mtime 写入 updated_at，实际：{}",
        updated_before
    );

    // 写入若干新消息，模拟 ingest_adapter 后续把消息批量写入。
    let h1 = blake3::hash(b"msg1").to_hex().to_string();
    let h2 = blake3::hash(b"msg2").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h1)
        .unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h2)
        .unwrap();

    let updated_after: String = {
        let conn = db.conn.lock().unwrap();
        conn.query_row(
            "SELECT updated_at FROM sessions WHERE id = ?1",
            params!["s1"],
            |row| row.get(0),
        )
        .unwrap()
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
        )
        .unwrap()
    };
    assert_eq!(count, 2, "insert_message 仍应维护 message_count 自增");
}

#[test]
fn test_fts_search() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
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
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    db.upsert_summary(super::SummaryUpsert {
        session_id: "s1",
        level: "L2_session",
        title: Some("Fix auth bug"),
        summary: "Fixed JWT parsing issue.",
        topics: &["auth".into()],
        decisions: &["use RS256".into()],
        message_count_at_creation: 10,
    })
    .unwrap();

    let summary = db.get_summary("s1", "L2_session").unwrap().unwrap();
    assert_eq!(summary.title.as_deref(), Some("Fix auth bug"));
    assert_eq!(summary.topics, vec!["auth"]);
    assert_eq!(summary.decisions, vec!["use RS256"]);

    db.upsert_summary(super::SummaryUpsert {
        session_id: "s1",
        level: "L2_session",
        title: Some("Updated title"),
        summary: "Updated summary.",
        topics: &["auth".into(), "jwt".into()],
        decisions: &[],
        message_count_at_creation: 20,
    })
    .unwrap();
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
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    let hash = blake3::hash(b"content").to_hex().to_string();
    db.insert_message("m1", "s1", "assistant", "content", None, 0, &hash)
        .unwrap();
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

    db.update_chunk_summary(chunk_id, "Implemented Redis caching.")
        .unwrap();

    let after = db.chunks_without_summary(10, 10).unwrap();
    assert!(after.is_empty());
}

#[test]
fn test_chunks_without_summary_respects_min_tokens() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "cursor", None, "/f.jsonl", 0, 0)
        .unwrap();
    let hash = blake3::hash(b"tiny").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "tiny", None, 0, &hash)
        .unwrap();
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
    assert!(
        results.is_empty(),
        "chunks below min_token_count should be excluded"
    );
}

#[test]
fn test_aggregate_summary_upsert_and_get() {
    let db = Db::open_in_memory().unwrap();
    db.upsert_aggregate_summary(super::AggregateSummaryUpsert {
        scope_type: "project",
        scope_key: "/my/project",
        title: Some("My Project"),
        summary: "Project-level summary.",
        topics: &["rust".into()],
        decisions: &["use FTS5".into()],
        session_count: 3,
    })
    .unwrap();

    let s = db
        .get_aggregate_summary("project", "/my/project")
        .unwrap()
        .unwrap();
    assert_eq!(s.title.as_deref(), Some("My Project"));
    assert_eq!(s.session_count, 3);

    db.upsert_aggregate_summary(super::AggregateSummaryUpsert {
        scope_type: "project",
        scope_key: "/my/project",
        title: Some("Updated Project"),
        summary: "Updated summary.",
        topics: &["rust".into(), "search".into()],
        decisions: &[],
        session_count: 5,
    })
    .unwrap();
    let updated = db
        .get_aggregate_summary("project", "/my/project")
        .unwrap()
        .unwrap();
    assert_eq!(updated.title.as_deref(), Some("Updated Project"));
    assert_eq!(updated.session_count, 5);
}

#[test]
fn test_aggregate_summary_not_found() {
    let db = Db::open_in_memory().unwrap();
    assert!(
        db.get_aggregate_summary("project", "nonexist")
            .unwrap()
            .is_none()
    );
}

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

    db.upsert_summary(super::SummaryUpsert {
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

    // 模拟 t=5s 又来了新消息。
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
/// session 已有摘要、且最近又有新消息 → 一边过期（应进）一边在冷却窗口（不该进）→ 不进；
/// 等会话冷却下来后再进。
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
    db.upsert_summary(super::SummaryUpsert {
        session_id: "s1",
        level: "L2_session",
        title: Some("t"),
        summary: "s",
        topics: &[],
        decisions: &[],
        message_count_at_creation: 2,
    })
    .unwrap();

    // 新消息进来 → 过期；同时 updated_at = now() → 在冷却窗口。
    db.insert_message("m3", "s1", "user", "msg3", None, 2, &h("msg3"))
        .unwrap();

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

    // 默认 NULL
    let row = &db.list_sessions_paged(10, 0).unwrap()[0];
    assert!(row.intent.is_none(), "新建 session 的 intent 默认为 NULL");

    // 写入并复读
    db.update_session_intent("s1", Some("修复登录")).unwrap();
    let row = &db.list_sessions_paged(10, 0).unwrap()[0];
    assert_eq!(row.intent.as_deref(), Some("修复登录"));

    // 写 None 应清空（重新生成摘要后 LLM 没给 intent 的情况）
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

/// 回归：`messages.timestamp` 为 NULL（cursor / continue_dev adapter）时
/// `get_session_detail` 必须用 `sessions.updated_at` 退化填充，前端 UI 才能
/// 始终渲染消息时间戳。如果不退化，user 会看到"消息没有时间"的旧版回归。
#[test]
fn test_get_session_detail_falls_back_to_session_updated_at_for_null_message_ts() {
    let db = Db::open_in_memory().unwrap();
    let session_updated_at = "2026-06-01 10:00:00";

    // 直接 INSERT，避免 insert_session/insert_message 改写 updated_at
    let conn = db.conn.lock().unwrap();
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

    let conn = db.conn.lock().unwrap();
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

/// v9 回归：新装库的 SCHEMA_SQL 必须把 `idx_chunks_has_summary` 和
/// `idx_messages_content_dedup` 全部建出来——这两个索引以前只在 v6
/// migration 里建过，从未写进 SCHEMA_SQL，导致新装库 / v6→v8 跳级升级
/// 的库都丢了这两个 hot path 索引。
#[test]
fn test_schema_sql_creates_all_hot_path_indexes() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND sql IS NOT NULL")
        .unwrap();
    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    drop(conn);

    let required = [
        "idx_messages_session_role_offset",
        "idx_summaries_session_level",
        "idx_sessions_updated_at",
        "idx_messages_timestamp",
        "idx_chunks_has_summary",
        "idx_messages_content_dedup",
    ];
    for idx in required {
        assert!(
            names.iter().any(|n| n == idx),
            "新装库缺少 hot path 索引 {idx}（现有：{:?}）",
            names
        );
    }
}

/// v9 关键路径：ingest 的 dedup 查询
/// `WHERE content_hash = ? AND session_id = ?` 必须命中
/// `idx_messages_content_dedup` 复合索引，否则在大 session 上退化到
/// 全 session 比对。
#[test]
fn test_message_dedup_uses_composite_index() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn.lock().unwrap();
    let plan: String = conn
        .query_row(
            "EXPLAIN QUERY PLAN SELECT 1 FROM messages WHERE content_hash = 'x' AND session_id = 'y'",
            [],
            |row| {
                // EXPLAIN QUERY PLAN 的 detail 列在第 4 个 (index 3)
                row.get::<_, String>(3)
            },
        )
        .unwrap();
    assert!(
        plan.contains("idx_messages_content_dedup"),
        "dedup 查询应走 idx_messages_content_dedup（实际 plan: {plan}）"
    );
}

/// v9 关键路径：chunks_with_summary_count 必须命中
/// `idx_chunks_has_summary` 局部索引，否则 70w+ chunks 真实库上
/// COUNT(*) 要 15+ 秒，会卡住「摘要进度百分比」展示。
#[test]
fn test_chunks_with_summary_count_uses_partial_index() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn.lock().unwrap();
    let plan: String = conn
        .query_row(
            "EXPLAIN QUERY PLAN SELECT COUNT(*) FROM chunks WHERE summary IS NOT NULL",
            [],
            |row| row.get::<_, String>(3),
        )
        .unwrap();
    assert!(
        plan.contains("idx_chunks_has_summary"),
        "chunks_with_summary_count 应走 idx_chunks_has_summary（实际 plan: {plan}）"
    );
}

/// v9 回归：老库从 v8 升到 v9 时，migration 必须补建那两个索引。
/// 模拟方法：手动 INSERT schema_version=8 然后 open，进 migration 路径。
#[test]
fn test_v9_migration_creates_missing_indexes() {
    use rusqlite::params;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("v8.db");

    // 第一步：建一个「假的 v8 老库」——
    // 我们没法真的回到 v8 schema，但可以模拟"丢了那两个索引"的状态：
    // 正常 open（已是 v9），然后手动 DROP 索引 + 把 version 回拨到 8，
    // 再次 open 时 migration 应该重新建出来。
    {
        let db = Db::open(&path).unwrap();
        let conn = db.conn.lock().unwrap();
        conn.execute("DROP INDEX IF EXISTS idx_chunks_has_summary", [])
            .unwrap();
        conn.execute("DROP INDEX IF EXISTS idx_messages_content_dedup", [])
            .unwrap();
        conn.execute("UPDATE schema_version SET version = ?1", params![8u32])
            .unwrap();
    }

    // 第二步：再 open 一次，触发 from=8 → 9 的 migration。
    let db = Db::open(&path).unwrap();
    let conn = db.conn.lock().unwrap();
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND sql IS NOT NULL")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        names.iter().any(|n| n == "idx_chunks_has_summary"),
        "v8→v9 migration 应补建 idx_chunks_has_summary"
    );
    assert!(
        names.iter().any(|n| n == "idx_messages_content_dedup"),
        "v8→v9 migration 应补建 idx_messages_content_dedup"
    );
    let version: u32 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    // v9 → 之后版本号会一路升到当前最新（v10 之后还可能继续上涨）。
    // 这里只断言"升到了至少 9"——v9 migration 跑过即可。
    assert!(
        version >= 9,
        "migration 后版本号应至少升到 9，实际 {}",
        version
    );
}

// ============================================================
// v10 / L5 「主题线索」回归测试
// ============================================================

use crate::storage::db::ThreadDraft;

fn seed_sessions(db: &Db, ids: &[&str]) {
    for id in ids {
        db.insert_session(id, "cursor", Some("/proj"), "/f", 0, 0)
            .unwrap();
    }
}

#[test]
fn test_v10_schema_has_threads_tables_and_indexes() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn.lock().unwrap();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(tables.contains(&"threads".to_string()));
    assert!(tables.contains(&"thread_sessions".to_string()));

    let indexes: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND sql IS NOT NULL")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        indexes.contains(&"idx_thread_sessions_session".to_string()),
        "thread_sessions(session_id) 索引必须存在"
    );
    assert!(
        indexes.contains(&"idx_threads_updated_at".to_string()),
        "threads(updated_at DESC) 索引必须存在"
    );
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
            // s2 / s3 不存在，应被外键拒绝，但不影响 s1
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
    // 最新插入的 b 应该排第一
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
    // thread_sessions 也应被 ON DELETE CASCADE 清掉
    let conn = db.conn.lock().unwrap();
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
    // 准备 2 个 session，分别属于不同项目 + 不同适配器，覆盖去重和分隔符。
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

    // 时间字段应在
    assert!(r.first_session_at.is_some(), "first_session_at 必须非空");
    assert!(r.last_session_at.is_some(), "last_session_at 必须非空");

    // 项目 / 适配器去重后应该有 2 个，按字母序
    assert_eq!(r.projects.len(), 2);
    assert!(r.projects.contains(&"/proj-a".to_string()));
    assert!(r.projects.contains(&"/proj-b".to_string()));
    assert_eq!(r.adapters.len(), 2);
    assert!(r.adapters.contains(&"claude_code".to_string()));
    assert!(r.adapters.contains(&"cursor".to_string()));

    // detail 通路也应一致
    let detail = db.get_thread_detail(r.id).unwrap().unwrap();
    assert_eq!(detail.thread.projects.len(), 2);
    assert_eq!(detail.thread.adapters.len(), 2);
}

/// v10 回归：把版本回拨到 9，再 open，应该自动建表。
#[test]
fn test_v10_migration_creates_threads_tables() {
    use rusqlite::params;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("v9.db");

    {
        let db = Db::open(&path).unwrap();
        let conn = db.conn.lock().unwrap();
        conn.execute("DROP TABLE IF EXISTS thread_sessions", [])
            .unwrap();
        conn.execute("DROP TABLE IF EXISTS threads", []).unwrap();
        conn.execute("UPDATE schema_version SET version = ?1", params![9u32])
            .unwrap();
    }

    let db = Db::open(&path).unwrap();
    let conn = db.conn.lock().unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(tables.contains(&"threads".to_string()));
    assert!(tables.contains(&"thread_sessions".to_string()));
    let version: u32 = conn
        .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, 10);
}
