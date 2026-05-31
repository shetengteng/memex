use super::Db;
use crate::storage::models::{Chunk, ChunkMetadata, ChunkType, SourceState};

#[test]
fn test_schema_init() {
    let db = Db::open_in_memory().unwrap();
    assert_eq!(db.session_count().unwrap(), 0);
    assert_eq!(db.chunk_count().unwrap(), 0);
}

#[test]
fn test_insert_session_and_message() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", Some("/proj"), "/path/file.jsonl")
        .unwrap();
    assert_eq!(db.session_count().unwrap(), 1);

    let hash = blake3::hash(b"hello world").to_hex().to_string();
    let inserted = db
        .insert_message("m1", "s1", "user", "hello world", None, 0, &hash)
        .unwrap();
    assert!(inserted);
    assert_eq!(db.message_count().unwrap(), 1);

    let dup = db
        .insert_message("m2", "s1", "user", "hello world", None, 0, &hash)
        .unwrap();
    assert!(!dup);
    assert_eq!(db.message_count().unwrap(), 1);
}

#[test]
fn test_insert_chunk_and_fts() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl").unwrap();
    let hash = blake3::hash(b"test content about redis pipeline").to_hex().to_string();
    db.insert_message(
        "m1",
        "s1",
        "assistant",
        "test content about redis pipeline",
        None,
        0,
        &hash,
    )
    .unwrap();

    let chunk = Chunk {
        id: None,
        message_id: "m1".to_string(),
        session_id: "s1".to_string(),
        chunk_type: ChunkType::Text,
        content: "test content about redis pipeline".to_string(),
        redacted_content: None,
        position: 0,
        token_count: 5,
        metadata: ChunkMetadata::default(),
    };
    let chunk_id = db.insert_chunk(&chunk).unwrap();
    assert!(chunk_id > 0);

    let results = db.fts_search("redis", 10).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].snippet.contains("redis"));
    assert_eq!(results[0].adapter.as_deref(), Some("claude_code"));
}

#[test]
fn test_source_offset() {
    let db = Db::open_in_memory().unwrap();
    let state = SourceState {
        adapter: "claude_code".to_string(),
        file_path: "/path/to/session.jsonl".to_string(),
        last_offset: 1024,
        last_mtime: 1717200000,
        last_scan: chrono::Utc::now(),
    };
    db.upsert_source(&state).unwrap();
    let offset = db.get_source_offset("/path/to/session.jsonl").unwrap();
    assert_eq!(offset, 1024);
}
