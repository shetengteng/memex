//! `tools/call` 的具体工具实现。
//!
//! 5c 起 mcp 不再直连 db：所有数据都通过 [`McpClient`] 向 daemon 拉。每次调
//! 用完成后再 POST `/mcp/log` 把 latency / args / result 异步沉淀，daemon
//! 这一头会顺手 `increment_metric(METRIC_MCP_CALLS)`，等价于老版本里手写的
//! `db.increment_metric(...)` + `db.insert_mcp_call(...)`。
//!
//! 隐私过滤（`processor::privacy::is_private_session`）仍在 mcp 这一侧做，
//! 因为它是纯函数（基于 project_path 字符串）且 daemon 的 `/search`、
//! `/sessions` 端点同时服务 GUI（不需要 mcp 那种"对外部 LLM 不暴露 personal
//! repo"语义）。所以保留是合理的，不算冗余直连 db。

use std::time::Instant;

use serde::Serialize;
use serde_json::Value;

use super::super::client::McpClient;
use super::super::protocol::{
    JsonRpcRequest, JsonRpcResponse, TOOL_GET_PROJECT_CONTEXT, TOOL_GET_SESSION, TOOL_LIST_RECENT,
    TOOL_LIST_SESSIONS_BY_RANGE, TOOL_SEARCH_MEMORY, TOOL_STATS,
};

pub(super) fn handle_tool_call(req: &JsonRpcRequest, client: &McpClient) -> JsonRpcResponse {
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

    let started = Instant::now();
    let result = match tool_name {
        TOOL_SEARCH_MEMORY => tool_search(client, &args),
        TOOL_GET_SESSION => tool_get_session(client, &args),
        TOOL_LIST_RECENT => tool_list_recent(client, &args),
        TOOL_STATS => tool_stats(client),
        TOOL_GET_PROJECT_CONTEXT => tool_get_project_context(client, &args),
        TOOL_LIST_SESSIONS_BY_RANGE => tool_list_sessions_by_range(client, &args),
        _ => Err(format!("unknown tool: {}", tool_name)),
    };
    let latency_ms = started.elapsed().as_millis() as u64;

    log_to_daemon(client, tool_name, latency_ms, &args, &result);

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

/// 把一次工具调用打到 daemon 的 /mcp/log。
///
/// daemon 端那一头会:
/// 1. 调 `truncate_payload` 把 args/result 截到合理大小
/// 2. `increment_metric(METRIC_MCP_CALLS)`
/// 3. `insert_mcp_call(...)` 入库
///
/// 任何步骤失败都被 mcp 这一头静默忽略 —— mcp 工具自身的成功 / 失败语义
/// 不能被 telemetry 影响。
fn log_to_daemon(
    client: &McpClient,
    tool: &str,
    latency_ms: u64,
    args: &Value,
    result: &std::result::Result<String, String>,
) {
    #[derive(Serialize)]
    struct McpLogPayload<'a> {
        tool: &'a str,
        latency_ms: u64,
        success: bool,
        error: Option<&'a str>,
        args: Option<String>,
        result: Option<String>,
    }

    let args_str = serde_json::to_string(args).ok();
    let (success, err_msg, result_str): (bool, Option<&str>, Option<String>) = match result {
        Ok(content) => {
            let wrapped = serde_json::to_string(&serde_json::json!({ "text": content }))
                .unwrap_or_else(|_| content.clone());
            (true, None, Some(wrapped))
        }
        Err(msg) => {
            let wrapped = serde_json::to_string(&serde_json::json!({ "error": msg }))
                .unwrap_or_else(|_| msg.clone());
            (false, Some(msg.as_str()), Some(wrapped))
        }
    };

    let payload = McpLogPayload {
        tool,
        latency_ms,
        success,
        error: err_msg,
        args: args_str,
        result: result_str,
    };
    let _ = client.post::<_, serde_json::Value>("/mcp/log", &payload);
}

fn deep_link_for_session(session_id: &str) -> String {
    format!("memex://session/{}", session_id)
}

fn enrich_session_value(value: &mut Value) {
    if let Some(obj) = value.as_object_mut()
        && let Some(id) = obj.get("id").and_then(|v| v.as_str())
    {
        let dl = deep_link_for_session(id);
        obj.insert("deep_link".to_string(), Value::String(dl));
    }
}

fn enrich_search_result(value: &mut Value) {
    if let Some(obj) = value.as_object_mut()
        && let Some(id) = obj.get("session_id").and_then(|v| v.as_str())
    {
        let dl = deep_link_for_session(id);
        obj.insert("deep_link".to_string(), Value::String(dl));
    }
}

fn tool_search(
    client: &McpClient,
    args: &Value,
) -> std::result::Result<String, String> {
    let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
    let limit_raw = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let limit_str = (limit_raw * 2).to_string();

    let mut q = vec![("q", query), ("limit", limit_str.as_str())];
    if let Some(adapter) = args.get("adapter").and_then(|v| v.as_str()) {
        q.push(("adapter", adapter));
    }
    if let Some(project) = args.get("project").and_then(|v| v.as_str()) {
        q.push(("project", project));
    }

    let resp: Value = client
        .get_with_query("/search", &q)
        .map_err(|e| e.to_string())?;
    let mut results = match resp.get("results").and_then(|v| v.as_array()) {
        Some(arr) => arr.clone(),
        None => return Ok("[]".to_string()),
    };

    results.retain(|item| {
        let sid = item.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
        let proj = item.get("project").and_then(|v| v.as_str());
        !memex_core::processor::privacy::is_private_session(sid, proj)
    });
    results.truncate(limit_raw);
    for item in results.iter_mut() {
        enrich_search_result(item);
    }

    serde_json::to_string_pretty(&Value::Array(results)).map_err(|e| e.to_string())
}

fn tool_get_session(
    client: &McpClient,
    args: &Value,
) -> std::result::Result<String, String> {
    let sid = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // session_id 长度 < 36 时先在 daemon 上做 prefix resolve（拉一批最新 session
    // 然后 client-side 前缀匹配，跟 CLI session.rs 的做法保持一致，避免在
    // daemon 上为 mcp 单独开个 endpoint）。
    let resolved = if sid.len() < 36 {
        let q = [("limit", "200")];
        let listing: Value = client
            .get_with_query("/sessions", &q)
            .map_err(|e| e.to_string())?;
        let matched = listing
            .get("sessions")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter()
                    .filter_map(|s| s.get("id").and_then(|v| v.as_str()))
                    .find(|id| id.starts_with(sid))
                    .map(String::from)
            });
        matched.unwrap_or_else(|| sid.to_string())
    } else {
        sid.to_string()
    };

    // daemon /sessions/{id} 返回 404 时映射成 mcp-friendly 错误。
    let mut detail: Value = client
        .get(&format!("/sessions/{}", resolved))
        .map_err(|e| format!("session not found: {} ({})", sid, e))?;

    enrich_session_value(&mut detail);
    serde_json::to_string_pretty(&detail).map_err(|e| e.to_string())
}

fn tool_list_recent(
    client: &McpClient,
    args: &Value,
) -> std::result::Result<String, String> {
    let limit_raw = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let limit_str = (limit_raw * 2).to_string();
    let q = [("limit", limit_str.as_str())];
    let resp: Value = client
        .get_with_query("/sessions", &q)
        .map_err(|e| e.to_string())?;

    let mut sessions = match resp.get("sessions").and_then(|v| v.as_array()) {
        Some(arr) => arr.clone(),
        None => return Ok("[]".to_string()),
    };
    sessions.retain(|s| {
        let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("");
        // daemon SessionEntry 用 rename_all=camelCase，注意字段是 `projectPath`。
        let proj = s.get("projectPath").and_then(|v| v.as_str());
        !memex_core::processor::privacy::is_private_session(id, proj)
    });
    sessions.truncate(limit_raw);
    for s in sessions.iter_mut() {
        enrich_session_value(s);
    }
    serde_json::to_string_pretty(&Value::Array(sessions)).map_err(|e| e.to_string())
}

fn tool_stats(client: &McpClient) -> std::result::Result<String, String> {
    let resp: Value = client.get("/stats").map_err(|e| e.to_string())?;
    let pretty = serde_json::json!({
        "sessions": resp.get("sessions").and_then(|v| v.as_i64()).unwrap_or(0),
        "messages": resp.get("messages").and_then(|v| v.as_i64()).unwrap_or(0),
        "chunks": resp.get("chunks").and_then(|v| v.as_i64()).unwrap_or(0),
    });
    serde_json::to_string_pretty(&pretty).map_err(|e| e.to_string())
}

fn tool_get_project_context(
    client: &McpClient,
    args: &Value,
) -> std::result::Result<String, String> {
    let mut q: Vec<(&str, &str)> = Vec::new();
    let top_str;
    if let Some(top) = args.get("top").and_then(|v| v.as_u64()) {
        top_str = top.to_string();
        q.push(("top", top_str.as_str()));
    }
    if let Some(project) = args.get("project").and_then(|v| v.as_str()) {
        q.push(("project", project));
    }
    let cwd_owned;
    if let Some(cwd) = args.get("cwd").and_then(|v| v.as_str()) {
        q.push(("cwd", cwd));
    } else if args.get("project").is_none() {
        // caller 没传 cwd 也没传 project：daemon 拿不到 mcp 客户端的 cwd（远端进程），
        // 所以 mcp 这一侧主动用自己的 cwd 注入，跟 CLI 行为一致。
        match std::env::current_dir() {
            Ok(c) => {
                cwd_owned = c.to_string_lossy().into_owned();
                q.push(("cwd", cwd_owned.as_str()));
            }
            Err(e) => return Err(format!("cannot determine cwd: {}", e)),
        }
    }

    let resp: Value = client
        .get_with_query("/context", &q)
        .map_err(|e| e.to_string())?;
    resp.get("markdown")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| "daemon returned no markdown".to_string())
}

fn tool_list_sessions_by_range(
    client: &McpClient,
    args: &Value,
) -> std::result::Result<String, String> {
    let after = args.get("after").and_then(|v| v.as_str()).unwrap_or("");
    let before = args.get("before").and_then(|v| v.as_str()).unwrap_or("");
    let limit_raw = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let limit_str = limit_raw.to_string();

    let mut q = vec![
        ("after", after),
        ("before", before),
        ("limit", limit_str.as_str()),
    ];
    if let Some(project) = args.get("project").and_then(|v| v.as_str()) {
        q.push(("project", project));
    }

    let resp: Value = client
        .get_with_query("/sessions/range", &q)
        .map_err(|e| e.to_string())?;

    // mcp 这一侧还要做最后一道隐私过滤 —— /sessions/range 给 GUI / CLI 用时
    // 不需要这层，所以 daemon 不带这个语义。
    let mut sessions = match resp.get("sessions").and_then(|v| v.as_array()) {
        Some(arr) => arr.clone(),
        None => return Ok("[]".to_string()),
    };
    sessions.retain(|s| {
        let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let proj = s.get("project_path").and_then(|v| v.as_str());
        !memex_core::processor::privacy::is_private_session(id, proj)
    });
    // 一次性给每条加 deep_link，跟旧 mcp 行为对齐。
    for s in sessions.iter_mut() {
        enrich_session_value(s);
    }
    let final_total = sessions.len();

    let out = serde_json::json!({
        "range": resp.get("range").cloned().unwrap_or(serde_json::json!({})),
        "total": final_total,
        "sessions": sessions,
    });
    serde_json::to_string_pretty(&out).map_err(|e| e.to_string())
}
