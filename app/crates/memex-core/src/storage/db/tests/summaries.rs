//! upsert/get/aggregate summary 以及 chunks_without_summary 阈值。

use crate::storage::db::{self, Db};
use crate::storage::models::{Chunk, ChunkMetadata, ChunkType};

#[test]
fn test_summary_upsert_and_get() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    db.upsert_summary(db::SummaryUpsert {
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

    db.upsert_summary(db::SummaryUpsert {
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
    db.upsert_aggregate_summary(db::AggregateSummaryUpsert {
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

    db.upsert_aggregate_summary(db::AggregateSummaryUpsert {
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
