//! JSON-RPC 方法路由与 protocol-level handler（`initialize` / `tools/list`）。
//!
//! `tools/call` 的具体工具实现见 [`super::tools`]。

use super::tools;
use crate::mcp::protocol::{
    JsonRpcRequest, JsonRpcResponse, TOOL_GET_PROJECT_CONTEXT, TOOL_GET_SESSION, TOOL_LIST_RECENT,
    TOOL_LIST_SESSIONS_BY_RANGE, TOOL_SEARCH_MEMORY, TOOL_STATS,
};
use crate::storage::db::Db;

pub(super) fn handle_request(req: &JsonRpcRequest, db: &Db) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req),
        "tools/list" => handle_list_tools(req),
        "tools/call" => tools::handle_tool_call(req, db),
        "notifications/initialized" => {
            JsonRpcResponse::success(req.id.clone(), serde_json::json!({}))
        }
        _ => JsonRpcResponse::error(
            req.id.clone(),
            -32601,
            &format!("method not found: {}", req.method),
        ),
    }
}

#[cfg(test)]
pub fn handle_request_for_test(req: &JsonRpcRequest, db: &Db) -> JsonRpcResponse {
    handle_request(req, db)
}

fn handle_initialize(req: &JsonRpcRequest) -> JsonRpcResponse {
    JsonRpcResponse::success(
        req.id.clone(),
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "memex", "version": env!("CARGO_PKG_VERSION") }
        }),
    )
}

fn handle_list_tools(req: &JsonRpcRequest) -> JsonRpcResponse {
    let tools = serde_json::json!({
        "tools": [
            {
                "name": TOOL_SEARCH_MEMORY,
                "description": "Search across all AI session history. Returns matching chunks with snippets.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "description": "Max results (default 5)" },
                        "adapter": { "type": "string", "description": "Filter by adapter" },
                        "project": { "type": "string", "description": "Filter by project" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": TOOL_GET_SESSION,
                "description": "Get a specific session with its messages by ID (full or prefix).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "Session ID or prefix" }
                    },
                    "required": ["session_id"]
                }
            },
            {
                "name": TOOL_LIST_RECENT,
                "description": "List recent AI sessions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "description": "Number of sessions (default 10)" }
                    }
                }
            },
            {
                "name": TOOL_STATS,
                "description": "Show Memex statistics (sessions, messages, chunks).",
                "inputSchema": { "type": "object", "properties": {} }
            },
            {
                "name": TOOL_GET_PROJECT_CONTEXT,
                "description": "Get a TARS-style 'work memory' summary for the current or specified project. Use this at the start of a session to recall what was done before, recent decisions, and likely next steps.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "cwd": { "type": "string", "description": "Current working directory used to resolve which project. Defaults to the matched session's cwd if omitted." },
                        "project": { "type": "string", "description": "Explicit project_path to look up. Bypasses cwd matching when provided." },
                        "top": { "type": "integer", "description": "Number of recent sessions to include (default 3)." }
                    }
                }
            },
            {
                "name": TOOL_LIST_SESSIONS_BY_RANGE,
                "description": "List sessions within a time range, with their L2 summaries. Useful for generating custom reports (daily/weekly) by AI.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "after": { "type": "string", "description": "Start date (ISO 8601, e.g. '2026-06-01' or '2026-06-01T00:00:00+00:00')" },
                        "before": { "type": "string", "description": "End date (ISO 8601, e.g. '2026-06-07' or '2026-06-07T23:59:59+00:00')" },
                        "limit": { "type": "integer", "description": "Max sessions to return (default 100)" },
                        "project": { "type": "string", "description": "Filter by project path" }
                    },
                    "required": ["after", "before"]
                }
            }
        ]
    });
    JsonRpcResponse::success(req.id.clone(), tools)
}
