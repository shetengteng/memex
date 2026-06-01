//! CLI JSON contract snapshot tests — verify serialized field presence.
//! If a field is removed or renamed, these tests will catch the regression.

use crate::storage::db::{Db, SessionRow, SessionDetail, MessageRow};
use crate::storage::models::{SearchResult, Chunk, ChunkType, ChunkMetadata};

fn setup_db() -> Db {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("sess-001", "claude_code", Some("/proj"), "/f.jsonl", 0, 0)
        .unwrap();
    let hash = blake3::hash(b"test content").to_hex().to_string();
    db.insert_message("msg-001", "sess-001", "user", "hello world", None, 0, &hash)
        .unwrap();
    db.insert_chunk(&Chunk {
        id: None,
        message_id: "msg-001".into(),
        session_id: "sess-001".into(),
        chunk_type: ChunkType::Text,
        content: "hello world".into(),
        redacted_content: None,
        position: 0,
        token_count: 3,
        metadata: ChunkMetadata::default(),
    })
    .unwrap();
    db
}

#[test]
fn test_search_result_json_fields() {
    let result = SearchResult {
        chunk_id: 1,
        session_id: "sess-001".into(),
        message_id: "msg-001".into(),
        chunk_type: "text".into(),
        content: "hello world".into(),
        snippet: "<mark>hello</mark> world".into(),
        rank: -3.5,
        match_reason: "keyword match: \"hello\"".into(),
        adapter: Some("claude_code".into()),
        project: Some("/proj".into()),
        timestamp: Some("2026-06-01T00:00:00+00:00".into()),
    };
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    let obj = json.as_object().unwrap();

    let required_fields = [
        "chunk_id", "session_id", "message_id", "chunk_type",
        "content", "snippet", "rank", "match_reason",
    ];
    for field in &required_fields {
        assert!(obj.contains_key(*field), "missing field: {}", field);
    }
    assert!(obj.contains_key("adapter"));
    assert!(obj.contains_key("project"));
    assert!(obj.contains_key("timestamp"));
}

#[test]
fn test_search_result_skips_none_fields() {
    let result = SearchResult {
        chunk_id: 1,
        session_id: "s".into(),
        message_id: "m".into(),
        chunk_type: "text".into(),
        content: "x".into(),
        snippet: "x".into(),
        rank: 0.0,
        match_reason: "".into(),
        adapter: None,
        project: None,
        timestamp: None,
    };
    let json: serde_json::Value = serde_json::to_value(&result).unwrap();
    let obj = json.as_object().unwrap();
    assert!(!obj.contains_key("adapter"), "None adapter should be skipped");
    assert!(!obj.contains_key("project"), "None project should be skipped");
    assert!(!obj.contains_key("timestamp"), "None timestamp should be skipped");
}

#[test]
fn test_session_row_json_fields() {
    let row = SessionRow {
        id: "sess-001".into(),
        source: "claude_code".into(),
        project_path: Some("/proj".into()),
        title: None,
        message_count: 5,
        created_at: "2026-06-01T00:00:00+00:00".into(),
        updated_at: "2026-06-01T00:00:00+00:00".into(),
        summary_title: None,
        first_user_message: Some("hello there".into()),
    };
    let json: serde_json::Value = serde_json::to_value(&row).unwrap();
    let obj = json.as_object().unwrap();
    let required = [
        "id",
        "source",
        "project_path",
        "message_count",
        "updated_at",
        "summary_title",
        "first_user_message",
    ];
    for field in &required {
        assert!(obj.contains_key(*field), "SessionRow missing: {}", field);
    }
}

#[test]
fn test_session_detail_json_fields() {
    let detail = SessionDetail {
        id: "sess-001".into(),
        source: "cursor".into(),
        project_path: Some("/proj".into()),
        file_path: "/f.jsonl".into(),
        title: Some("Test session".into()),
        summary: None,
        topics: Vec::new(),
        decisions: Vec::new(),
        created_at: "2026-06-01T00:00:00+00:00".into(),
        updated_at: "2026-06-01T01:00:00+00:00".into(),
        message_count: 2,
        messages: vec![MessageRow {
            id: "msg-001".into(),
            role: "user".into(),
            content: "hello".into(),
            timestamp: Some("2026-06-01T00:00:00+00:00".into()),
        }],
    };
    let json: serde_json::Value = serde_json::to_value(&detail).unwrap();
    let obj = json.as_object().unwrap();
    let required = [
        "id", "source", "project_path", "file_path", "title",
        "summary", "topics", "decisions",
        "created_at", "updated_at", "message_count", "messages",
    ];
    for field in &required {
        assert!(obj.contains_key(*field), "SessionDetail missing: {}", field);
    }
    let messages = obj["messages"].as_array().unwrap();
    let msg = messages[0].as_object().unwrap();
    for field in &["id", "role", "content", "timestamp"] {
        assert!(msg.contains_key(*field), "MessageRow missing: {}", field);
    }
}

#[test]
fn test_stats_json_contract() {
    let db = setup_db();
    let sessions = db.session_count().unwrap();
    let messages = db.message_count().unwrap();
    let chunks = db.chunk_count().unwrap();
    let stats = serde_json::json!({
        "sessions": sessions,
        "messages": messages,
        "chunks": chunks,
    });
    let obj = stats.as_object().unwrap();
    for field in &["sessions", "messages", "chunks"] {
        assert!(obj.contains_key(*field), "stats missing: {}", field);
        assert!(obj[*field].is_number(), "stats.{} should be number", field);
    }
}

#[test]
fn test_chunk_metadata_json_fields() {
    let meta = ChunkMetadata {
        topics: vec!["redis".into()],
        languages: vec!["rust".into()],
        has_code: true,
        tools_used: vec!["Read".into()],
        error_keywords: vec![],
    };
    let json: serde_json::Value = serde_json::to_value(&meta).unwrap();
    let obj = json.as_object().unwrap();
    let required = ["topics", "languages", "has_code", "tools_used", "error_keywords"];
    for field in &required {
        assert!(obj.contains_key(*field), "ChunkMetadata missing: {}", field);
    }
    assert!(obj["topics"].is_array());
    assert!(obj["has_code"].is_boolean());
}
