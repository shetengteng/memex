//! Schema bring-up + insert / dedup / fts / source_offset / kv 基础冒烟。

use std::sync::Arc;

use crate::clock::FrozenClock;
use crate::storage::db::Db;
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
        let conn = db.conn.lock();
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

    let h1 = blake3::hash(b"msg1").to_hex().to_string();
    let h2 = blake3::hash(b"msg2").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "msg1", None, 0, &h1)
        .unwrap();
    db.insert_message("m2", "s1", "assistant", "msg2", None, 1, &h2)
        .unwrap();

    let updated_after: String = {
        let conn = db.conn.lock();
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

    let count: i64 = {
        let conn = db.conn.lock();
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

/// 验证 [`crate::clock::FrozenClock`] 通过 [`Db::open_in_memory_with_clock`]
/// 注入后，所有内部走 `self.now_utc()` 的写路径都拿到固定时间戳。
///
/// 这条用例同时充当 Clock trait 的 contract regression：未来如果有人
/// 把某个 Db 内部 `chrono::Utc::now()` 的位置忘了改成 `self.now_utc()`，
/// 时间戳会不等于 frozen 锚点，断言立刻失败。
#[test]
fn db_with_frozen_clock_pins_session_timestamps() {
    use rusqlite::params;

    let frozen = Arc::new(FrozenClock::epoch_2026());
    let expected = "2026-01-01T00:00:00+00:00";
    let db = Db::open_in_memory_with_clock(frozen).unwrap();

    db.insert_session("s1", "cursor", Some("/proj"), "/f.jsonl", 0, 0)
        .unwrap();

    let (created, updated): (String, String) = {
        let conn = db.conn.lock();
        conn.query_row(
            "SELECT created_at, updated_at FROM sessions WHERE id = ?1",
            params!["s1"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    };

    assert_eq!(created, expected, "created_at must come from frozen clock");
    assert_eq!(updated, expected, "updated_at must come from frozen clock");
}
