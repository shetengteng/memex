use std::io::{self, BufRead, Write};

use anyhow::Result;

use super::protocol::*;
use crate::retriever::{Retriever, SearchFilter};
use crate::storage::db::Db;

pub fn run_stdio(db: &Db) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(None, -32700, &format!("parse error: {}", e));
                write_response(&mut out, &resp)?;
                continue;
            }
        };

        let response = handle_request(&request, db);
        write_response(&mut out, &response)?;
    }

    Ok(())
}

fn write_response(out: &mut impl Write, resp: &JsonRpcResponse) -> Result<()> {
    let json = serde_json::to_string(resp)?;
    writeln!(out, "{}", json)?;
    out.flush()?;
    Ok(())
}

fn handle_request(req: &JsonRpcRequest, db: &Db) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req),
        "tools/list" => handle_list_tools(req),
        "tools/call" => handle_tool_call(req, db),
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
            }
        ]
    });
    JsonRpcResponse::success(req.id.clone(), tools)
}

fn handle_tool_call(req: &JsonRpcRequest, db: &Db) -> JsonRpcResponse {
    let tool_name = req
        .params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let args = req
        .params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let _ = db.increment_metric(crate::storage::metrics::METRIC_MCP_CALLS);

    let result = match tool_name {
        TOOL_SEARCH_MEMORY => tool_search(db, &args),
        TOOL_GET_SESSION => tool_get_session(db, &args),
        TOOL_LIST_RECENT => tool_list_recent(db, &args),
        TOOL_STATS => tool_stats(db),
        _ => Err(format!("unknown tool: {}", tool_name)),
    };

    match result {
        Ok(content) => JsonRpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "content": [{ "type": "text", "text": content }]
            }),
        ),
        Err(msg) => JsonRpcResponse::success(
            req.id.clone(),
            serde_json::json!({
                "content": [{ "type": "text", "text": msg }],
                "isError": true
            }),
        ),
    }
}

fn tool_search(db: &Db, args: &serde_json::Value) -> std::result::Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let filter = SearchFilter {
        adapter: args
            .get("adapter")
            .and_then(|v| v.as_str())
            .map(String::from),
        project: args
            .get("project")
            .and_then(|v| v.as_str())
            .map(String::from),
        ..Default::default()
    };

    let retriever = Retriever::new(db);
    let mut results = retriever
        .search_filtered(query, limit * 2, &filter)
        .map_err(|e| e.to_string())?;
    results.retain(|r| {
        !crate::processor::privacy::is_private_session(&r.session_id, r.project.as_deref())
    });
    results.truncate(limit);
    serde_json::to_string_pretty(&results).map_err(|e| e.to_string())
}

fn tool_get_session(db: &Db, args: &serde_json::Value) -> std::result::Result<String, String> {
    let sid = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let resolved = if sid.len() < 36 {
        db.find_session_by_prefix(sid)
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| sid.to_string())
    } else {
        sid.to_string()
    };
    let detail = db
        .get_session_detail(&resolved)
        .map_err(|e| e.to_string())?;
    match detail {
        Some(d) => serde_json::to_string_pretty(&d).map_err(|e| e.to_string()),
        None => Err(format!("session not found: {}", sid)),
    }
}

fn tool_list_recent(db: &Db, args: &serde_json::Value) -> std::result::Result<String, String> {
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let mut sessions = db.list_sessions(limit * 2).map_err(|e| e.to_string())?;
    sessions.retain(|s| {
        !crate::processor::privacy::is_private_session(&s.id, s.project_path.as_deref())
    });
    sessions.truncate(limit);
    serde_json::to_string_pretty(&sessions).map_err(|e| e.to_string())
}

fn tool_stats(db: &Db) -> std::result::Result<String, String> {
    let stats = serde_json::json!({
        "sessions": db.session_count().unwrap_or(0),
        "messages": db.message_count().unwrap_or(0),
        "chunks": db.chunk_count().unwrap_or(0),
    });
    serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())
}
