//! MCP 工具契约测试 —— 校验 JSON-RPC 请求 / 响应的字段结构。

use super::protocol::*;
use super::server::handle_request_for_test;
use crate::storage::db::Db;
use crate::storage::models::{Chunk, ChunkMetadata, ChunkType};

fn setup_db() -> Db {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("sess-001", "claude_code", Some("/proj"), "/f.jsonl", 0, 0)
        .unwrap();
    let hash = blake3::hash(b"hello redis").to_hex().to_string();
    db.insert_message("msg-001", "sess-001", "user", "hello redis", None, 0, &hash)
        .unwrap();
    db.insert_chunk(&Chunk {
        id: None,
        message_id: "msg-001".into(),
        session_id: "sess-001".into(),
        chunk_type: ChunkType::Text,
        content: "hello redis pipeline".into(),
        redacted_content: None,
        position: 0,
        token_count: 5,
        metadata: ChunkMetadata::default(),
    })
    .unwrap();
    db
}

fn make_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(1)),
        method: method.to_string(),
        params,
    }
}

#[test]
fn test_initialize() {
    let db = setup_db();
    let req = make_request("initialize", serde_json::json!({}));
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert!(result["serverInfo"]["name"] == "memex");
}

#[test]
fn test_tools_list() {
    let db = setup_db();
    let req = make_request("tools/list", serde_json::json!({}));
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_none());
    let tools = resp.result.unwrap();
    let tool_list = tools["tools"].as_array().unwrap();
    assert_eq!(tool_list.len(), 6);
    let names: Vec<&str> = tool_list.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"search_memory"));
    assert!(names.contains(&"get_session"));
    assert!(names.contains(&"list_recent"));
    assert!(names.contains(&"stats"));
    assert!(names.contains(&"get_project_context"));
    assert!(names.contains(&"list_sessions_by_range"));
}

#[test]
fn test_tool_search_memory() {
    let db = setup_db();
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "search_memory",
            "arguments": { "query": "redis", "limit": 5 }
        }),
    );
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(content).unwrap();
    assert!(!parsed.is_empty());
    assert!(parsed[0]["session_id"].as_str().unwrap().contains("sess-001"));
    assert_eq!(
        parsed[0]["deep_link"].as_str().unwrap(),
        "memex://session/sess-001"
    );
}

#[test]
fn test_tool_get_session() {
    let db = setup_db();
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "get_session",
            "arguments": { "session_id": "sess-001" }
        }),
    );
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(content).unwrap();
    assert_eq!(parsed["id"], "sess-001");
    assert_eq!(parsed["source"], "claude_code");
    assert_eq!(parsed["deep_link"], "memex://session/sess-001");
}

#[test]
fn test_tool_list_recent() {
    let db = setup_db();
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "list_recent",
            "arguments": { "limit": 10 }
        }),
    );
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(content).unwrap();
    assert!(!parsed.is_empty());
    assert_eq!(parsed[0]["deep_link"], "memex://session/sess-001");
}

#[test]
fn test_tool_stats() {
    let db = setup_db();
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "stats",
            "arguments": {}
        }),
    );
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(content).unwrap();
    assert_eq!(parsed["sessions"], 1);
    assert_eq!(parsed["messages"], 1);
    assert_eq!(parsed["chunks"], 1);
}

#[test]
fn test_unknown_method() {
    let db = setup_db();
    let req = make_request("unknown/method", serde_json::json!({}));
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32601);
}

#[test]
fn test_tool_get_project_context_with_explicit_project() {
    let db = setup_db(); // 已经 seed 了 /proj 项目下一条 session
    // 给 session 补一条 user 消息确保 message_count >= 2，再上一条 L2 摘要
    let hash = blake3::hash(b"ack").to_hex().to_string();
    db.insert_message("msg-002", "sess-001", "assistant", "ack", None, 1, &hash).unwrap();
    db.upsert_summary(
        "sess-001", "L2_session",
        Some("Redis pipeline talk"),
        "Discussion of using redis pipeline for batching",
        &["redis".into()], &["batch writes via pipeline".into()],
        2,
    ).unwrap();

    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "get_project_context",
            "arguments": { "project": "/proj", "top": 3 }
        }),
    );
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_none(), "tool error: {:?}", resp.error);
    let result = resp.result.unwrap();
    let content = result["content"][0]["text"].as_str().unwrap();
    assert!(content.contains("**Memex 工作记忆**"), "missing banner:\n{}", content);
    assert!(content.contains("**proj**"), "missing project name:\n{}", content);
    assert!(content.contains("Redis pipeline talk"), "missing L2 title:\n{}", content);
    assert!(content.contains("batch writes via pipeline"), "missing decision:\n{}", content);
}

#[test]
fn test_tool_get_project_context_returns_banner_when_no_match() {
    let db = Db::open_in_memory().unwrap();
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "get_project_context",
            "arguments": { "cwd": "/nonexistent/path" }
        }),
    );
    let resp = handle_request_for_test(&req, &db);
    assert!(resp.error.is_none(), "MCP tool 不该报 error，应返回 banner");
    let content = resp.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(content.contains("**Memex 工作记忆**"));
    assert!(
        content.contains("/nonexistent/path"),
        "banner 应展示出用户传入的 cwd:\n{}",
        content,
    );
}

#[test]
fn test_tools_list_includes_get_project_context() {
    let db = setup_db();
    let req = make_request("tools/list", serde_json::json!({}));
    let resp = handle_request_for_test(&req, &db);
    let names: Vec<String> = resp.result.unwrap()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert!(
        names.contains(&"get_project_context".to_string()),
        "工具列表应包含 get_project_context，实际：{:?}",
        names
    );
}
