//! MCP 协议层契约测试。
//!
//! 5c 起 mcp 不再直连 db；`tools/call` 的端到端覆盖由 daemon 端 routes
//! 测试 + 部署验证负责，本文件只测**协议层 / dispatch 行为**（不需要任何
//! fixture data）：
//! * `initialize` 返回正确的 protocolVersion / serverInfo
//! * `tools/list` 返回完整的 6 个工具
//! * `tools/list` 的 schema 字段齐全（required / properties / type）
//! * 未知方法返回 JSON-RPC `-32601 method not found`
//! * `tools/call` 在没有 client 的测试入口下被拒（保持契约：tools/call 必须
//!   被 routed 到 client handler）

use super::protocol::*;
use super::server::handle_protocol_request_for_test;

fn make_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(serde_json::json!(1)),
        method: method.to_string(),
        params,
    }
}

#[test]
fn test_initialize_returns_protocol_version() {
    let req = make_request("initialize", serde_json::json!({}));
    let resp = handle_protocol_request_for_test(&req);
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "memex");
}

#[test]
fn test_tools_list_returns_six_tools() {
    let req = make_request("tools/list", serde_json::json!({}));
    let resp = handle_protocol_request_for_test(&req);
    assert!(resp.error.is_none());
    let tools = resp.result.unwrap();
    let tool_list = tools["tools"].as_array().unwrap();
    assert_eq!(tool_list.len(), 6);
    let names: Vec<&str> = tool_list
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for expected in [
        "search_memory",
        "get_session",
        "list_recent",
        "stats",
        "get_project_context",
        "list_sessions_by_range",
    ] {
        assert!(
            names.contains(&expected),
            "tool {} missing from list: {:?}",
            expected,
            names
        );
    }
}

#[test]
fn test_tools_list_schemas_have_required_fields() {
    let req = make_request("tools/list", serde_json::json!({}));
    let resp = handle_protocol_request_for_test(&req);
    let tools = resp.result.unwrap()["tools"].as_array().unwrap().clone();

    // search_memory: 必须有 inputSchema.required = ["query"]
    let search = tools
        .iter()
        .find(|t| t["name"] == "search_memory")
        .unwrap();
    let req_arr = search["inputSchema"]["required"].as_array().unwrap();
    assert!(
        req_arr.iter().any(|v| v == "query"),
        "search_memory required must contain 'query'"
    );

    // list_sessions_by_range: 必须有 inputSchema.required = ["after","before"]
    let range = tools
        .iter()
        .find(|t| t["name"] == "list_sessions_by_range")
        .unwrap();
    let req_arr = range["inputSchema"]["required"].as_array().unwrap();
    assert!(req_arr.iter().any(|v| v == "after"));
    assert!(req_arr.iter().any(|v| v == "before"));
}

#[test]
fn test_unknown_method_returns_method_not_found() {
    let req = make_request("unknown/method", serde_json::json!({}));
    let resp = handle_protocol_request_for_test(&req);
    assert!(resp.error.is_some());
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32601);
    assert!(err.message.contains("unknown/method"));
}

#[test]
fn test_notifications_initialized_returns_ack() {
    let req = make_request("notifications/initialized", serde_json::json!({}));
    let resp = handle_protocol_request_for_test(&req);
    assert!(resp.error.is_none());
    // 协议要求 ack 一个空对象，调用方靠 id 区分 request/response。
    assert!(resp.result.is_some());
}

#[test]
fn test_tools_call_is_routed_to_client_handler() {
    // 协议层入口必须不直接处理 tools/call（必须依赖 client）。
    // handle_protocol_request_for_test 会把 tools/call 转成 -32601 兜底响应。
    let req = make_request(
        "tools/call",
        serde_json::json!({
            "name": "stats",
            "arguments": {}
        }),
    );
    let resp = handle_protocol_request_for_test(&req);
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32601);
}
