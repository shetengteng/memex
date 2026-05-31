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
    db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl")
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

#[test]
fn test_fts_search() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl").unwrap();
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
    db.insert_session("s1", "claude_code", None, "/f.jsonl").unwrap();
    db.upsert_summary(
        "s1", "L2_session", Some("Fix auth bug"),
        "Fixed JWT parsing issue.", &["auth".into()], &["use RS256".into()],
    ).unwrap();

    let summary = db.get_summary("s1", "L2_session").unwrap().unwrap();
    assert_eq!(summary.title.as_deref(), Some("Fix auth bug"));
    assert_eq!(summary.topics, vec!["auth"]);
    assert_eq!(summary.decisions, vec!["use RS256"]);

    db.upsert_summary(
        "s1", "L2_session", Some("Updated title"),
        "Updated summary.", &["auth".into(), "jwt".into()], &[],
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
