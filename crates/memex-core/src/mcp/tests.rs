//! MCP tool contract tests — validate JSON-RPC request/response shapes.

use super::protocol::*;
use super::server::handle_request_for_test;
use crate::storage::db::Db;
use crate::storage::models::{Chunk, ChunkMetadata, ChunkType};

fn setup_db() -> Db {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("sess-001", "claude_code", Some("/proj"), "/f.jsonl")
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
    assert_eq!(tool_list.len(), 4);
    let names: Vec<&str> = tool_list.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"search_memory"));
    assert!(names.contains(&"get_session"));
    assert!(names.contains(&"list_recent"));
    assert!(names.contains(&"stats"));
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
