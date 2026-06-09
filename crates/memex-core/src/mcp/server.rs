use std::io::{self, BufRead, Write};
use std::time::Instant;

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
        TOOL_GET_PROJECT_CONTEXT => tool_get_project_context(db, &args),
        TOOL_LIST_SESSIONS_BY_RANGE => tool_list_sessions_by_range(db, &args),
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

fn deep_link_for_session(session_id: &str) -> String {
    format!("memex://session/{}", session_id)
}

fn enrich_session_value(value: &mut serde_json::Value) {
    if let Some(obj) = value.as_object_mut() {
        if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
            let dl = deep_link_for_session(id);
            obj.insert("deep_link".to_string(), serde_json::Value::String(dl));
        }
    }
}

fn enrich_search_result(value: &mut serde_json::Value) {
    if let Some(obj) = value.as_object_mut() {
        if let Some(id) = obj.get("session_id").and_then(|v| v.as_str()) {
            let dl = deep_link_for_session(id);
            obj.insert("deep_link".to_string(), serde_json::Value::String(dl));
        }
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

    let started = Instant::now();
    let retriever = Retriever::new(db);
    let mut results = retriever
        .search_filtered(query, limit * 2, &filter)
        .map_err(|e| e.to_string())?;
    results.retain(|r| {
        !crate::processor::privacy::is_private_session(&r.session_id, r.project.as_deref())
    });
    results.truncate(limit);

    let latency_ms = started.elapsed().as_millis() as u64;
    let _ = db.write_access_log(query, results.len(), latency_ms);
    let _ = db.record_search_latency(latency_ms);

    let mut value = serde_json::to_value(&results).map_err(|e| e.to_string())?;
    if let Some(arr) = value.as_array_mut() {
        for item in arr.iter_mut() {
            enrich_search_result(item);
        }
    }
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
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
        Some(d) => {
            let mut value = serde_json::to_value(&d).map_err(|e| e.to_string())?;
            enrich_session_value(&mut value);
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
        }
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
    let mut value = serde_json::to_value(&sessions).map_err(|e| e.to_string())?;
    if let Some(arr) = value.as_array_mut() {
        for item in arr.iter_mut() {
            enrich_session_value(item);
        }
    }
    serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
}

fn tool_stats(db: &Db) -> std::result::Result<String, String> {
    let stats = serde_json::json!({
        "sessions": db.session_count().unwrap_or(0),
        "messages": db.message_count().unwrap_or(0),
        "chunks": db.chunk_count().unwrap_or(0),
    });
    serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())
}

/// MCP 工具：把项目工作记忆按 TARS-style Markdown 返回给 AI。
///
/// 与 `memex context` CLI 同源，只是入口走 MCP；用法：
/// - 没有 hook 的环境（OpenCode）/ 用户禁用了 hook → 在 system prompt
///   或 skill 里指引 AI 第一轮主动调一下这个工具
/// - hook 的 fallback：hook 因任何原因失败，AI 还能手动找回
fn tool_get_project_context(
    db: &Db,
    args: &serde_json::Value,
) -> std::result::Result<String, String> {
    use crate::context::{ContextOptions, build_context, search_by_project};
    use std::path::PathBuf;

    let top = args.get("top").and_then(|v| v.as_u64()).unwrap_or(3) as usize;

    let project_path = if let Some(p) = args.get("project").and_then(|v| v.as_str()) {
        p.to_string()
    } else {
        let cwd = args
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or_else(|| "no cwd or project provided".to_string())?;
        match search_by_project(db, &cwd).map_err(|e| e.to_string())? {
            Some(m) => m.project_path,
            None => {
                // 找不到匹配项目 → 返回一个 banner 而非报 error。
                // 这样调用方（AI）能正常理解"目前没有相关记忆"，并据此调整后续提问。
                return Ok(format!(
                    "**Memex 工作记忆**\n\n当前目录 {} 暂无关联项目会话；新的 AI 会话会被自动采集。",
                    cwd.display()
                ));
            }
        }
    };

    // MCP 这条路径默认开启脱敏 —— 走 MCP 的请求来自被注入到云端 LLM 的会话，
    // 跟 hook 路径相同的安全边界。
    let md = build_context(
        db,
        &project_path,
        &ContextOptions {
            top_n: top,
            redact: true,
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(md)
}

fn tool_list_sessions_by_range(
    db: &Db,
    args: &serde_json::Value,
) -> std::result::Result<String, String> {
    let after_raw = args.get("after").and_then(|v| v.as_str()).unwrap_or("");
    let before_raw = args.get("before").and_then(|v| v.as_str()).unwrap_or("");
    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let project_filter = args.get("project").and_then(|v| v.as_str());

    let after = normalize_date_bound(after_raw, true);
    let before = normalize_date_bound(before_raw, false);

    let mut sessions = db
        .list_sessions_in_range(&after, &before)
        .map_err(|e| e.to_string())?;

    sessions.retain(|s| {
        !crate::processor::privacy::is_private_session(&s.id, s.project_path.as_deref())
    });
    if let Some(proj) = project_filter {
        sessions.retain(|s| s.project_path.as_deref() == Some(proj));
    }
    sessions.truncate(limit);

    let mut enriched: Vec<serde_json::Value> = Vec::with_capacity(sessions.len());
    for s in &sessions {
        let summary = db.get_summary(&s.id, "L2_session").ok().flatten();
        let mut obj = serde_json::json!({
            "id": s.id,
            "source": s.source,
            "project_path": s.project_path,
            "title": s.title,
            "message_count": s.message_count,
            "updated_at": s.updated_at,
            "deep_link": deep_link_for_session(&s.id),
        });
        if let Some(sum) = summary {
            obj["l2_summary"] = serde_json::json!({
                "title": sum.title,
                "summary": sum.summary,
                "topics": sum.topics,
                "decisions": sum.decisions,
            });
        }
        enriched.push(obj);
    }

    let result = serde_json::json!({
        "range": { "after": after, "before": before },
        "total": enriched.len(),
        "sessions": enriched,
    });
    serde_json::to_string_pretty(&result).map_err(|e| e.to_string())
}

fn normalize_date_bound(raw: &str, is_start: bool) -> String {
    if raw.contains('T') {
        return raw.to_string();
    }
    if is_start {
        format!("{}T00:00:00+00:00", raw)
    } else {
        format!("{}T23:59:59+00:00", raw)
    }
}
