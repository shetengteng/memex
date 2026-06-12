//! CLI JSON 契约的快照测试 —— 检查序列化字段是否齐全。
//! 一旦字段被删除或重命名，这些测试会立刻把回归捞出来。

use crate::storage::db::{Db, MessageRow, SessionDetail, SessionRow};
use crate::storage::models::{Chunk, ChunkMetadata, ChunkType, SearchResult};

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
        "chunk_id",
        "session_id",
        "message_id",
        "chunk_type",
        "content",
        "snippet",
        "rank",
        "match_reason",
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
    assert!(
        !obj.contains_key("adapter"),
        "None adapter should be skipped"
    );
    assert!(
        !obj.contains_key("project"),
        "None project should be skipped"
    );
    assert!(
        !obj.contains_key("timestamp"),
        "None timestamp should be skipped"
    );
}

/// SessionRow 的 IPC 序列化形态：所有多词字段都是 camelCase
/// （`projectPath` / `messageCount` / `createdAt` / `updatedAt` /
/// `summaryTitle` / `firstUserMessage`），snake_case 字段在 JSON 里
/// 不再存在。前端 `app/desktop/src/types/index.ts::SessionRow` 必须
/// 与此断言保持一致。
///
/// 同时验证 SessionRow 仍能从 snake_case 形式（来自 SQL 列名经
/// `serde_rusqlite::from_rows`）反序列化 —— 这条用例守住 alias 的退路，
/// 防止有人在 dto.rs 上误删 `#[serde(alias = "...")]` 后 SQL 路径静默崩。
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
        intent: Some("修复 popup 列表中 intent 字段".into()),
    };
    let json: serde_json::Value = serde_json::to_value(&row).unwrap();
    let obj = json.as_object().unwrap();
    let required = [
        "id",
        "source",
        "projectPath",
        "messageCount",
        "createdAt",
        "updatedAt",
        "summaryTitle",
        "firstUserMessage",
        "intent",
    ];
    for field in &required {
        assert!(obj.contains_key(*field), "SessionRow missing: {}", field);
    }
    for legacy in &[
        "project_path",
        "message_count",
        "created_at",
        "updated_at",
        "summary_title",
        "first_user_message",
    ] {
        assert!(
            !obj.contains_key(*legacy),
            "SessionRow leaked snake_case key: {}",
            legacy
        );
    }

    // SQL 列名是 snake_case，alias 必须接受它 —— 否则 from_rows::<SessionRow>
    // 会在所有 list_sessions / list_recent / get_session 路径上 silently 翻车。
    let from_snake: SessionRow = serde_json::from_value(serde_json::json!({
        "id": "sess-002",
        "source": "cursor",
        "project_path": "/proj-snake",
        "title": null,
        "message_count": 7,
        "created_at": "2026-06-01T00:00:00+00:00",
        "updated_at": "2026-06-01T00:00:00+00:00",
        "summary_title": null,
        "first_user_message": "snake",
        "intent": null,
    }))
    .expect("SessionRow must accept snake_case input via #[serde(alias)] for SQL row mapping");
    assert_eq!(from_snake.project_path.as_deref(), Some("/proj-snake"));
    assert_eq!(from_snake.message_count, 7);
}

/// SessionDetail 的 IPC 序列化形态：与 SessionRow 同步走 camelCase。
/// 多词字段为 `projectPath` / `filePath` / `createdAt` / `updatedAt` /
/// `messageCount`；snake_case 不再出现。
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
        intent: Some("调研 intent 字段补全".into()),
    };
    let json: serde_json::Value = serde_json::to_value(&detail).unwrap();
    let obj = json.as_object().unwrap();
    let required = [
        "id",
        "source",
        "projectPath",
        "filePath",
        "title",
        "summary",
        "topics",
        "decisions",
        "createdAt",
        "updatedAt",
        "messageCount",
        "messages",
        "intent",
    ];
    for field in &required {
        assert!(obj.contains_key(*field), "SessionDetail missing: {}", field);
    }
    for legacy in &[
        "project_path",
        "file_path",
        "created_at",
        "updated_at",
        "message_count",
    ] {
        assert!(
            !obj.contains_key(*legacy),
            "SessionDetail leaked snake_case key: {}",
            legacy
        );
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
    let required = [
        "topics",
        "languages",
        "has_code",
        "tools_used",
        "error_keywords",
    ];
    for field in &required {
        assert!(obj.contains_key(*field), "ChunkMetadata missing: {}", field);
    }
    assert!(obj["topics"].is_array());
    assert!(obj["has_code"].is_boolean());
}
