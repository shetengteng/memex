//! `tools/call` 的具体工具实现。每个 `tool_*` 函数对应一个 MCP tool，
//! 输入是 JSON value（caller 已经从 JSON-RPC 里解出 `params.arguments`），
//! 输出是 pretty JSON 字符串（按 MCP content text 包装）。
//!
//! Tool 元数据（name / description / inputSchema）声明在 [`super::dispatch`]
//! 的 `handle_list_tools` 里。

use std::time::Instant;

use crate::protocol::{
    JsonRpcRequest, JsonRpcResponse, TOOL_GET_PROJECT_CONTEXT, TOOL_GET_SESSION, TOOL_LIST_RECENT,
    TOOL_LIST_SESSIONS_BY_RANGE, TOOL_SEARCH_MEMORY, TOOL_STATS,
};
use memex_core::retriever::{Retriever, SearchFilter};
use memex_core::storage::db::Db;

pub(super) fn handle_tool_call(req: &JsonRpcRequest, db: &Db) -> JsonRpcResponse {
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

    let _ = db.increment_metric(memex_core::storage::metrics::METRIC_MCP_CALLS);

    let started = Instant::now();
    let result = match tool_name {
        TOOL_SEARCH_MEMORY => tool_search(db, &args),
        TOOL_GET_SESSION => tool_get_session(db, &args),
        TOOL_LIST_RECENT => tool_list_recent(db, &args),
        TOOL_STATS => tool_stats(db),
        TOOL_GET_PROJECT_CONTEXT => tool_get_project_context(db, &args),
        TOOL_LIST_SESSIONS_BY_RANGE => tool_list_sessions_by_range(db, &args),
        _ => Err(format!("unknown tool: {}", tool_name)),
    };
    let latency_ms = started.elapsed().as_millis() as u64;

    // 写一行 mcp_call_log。失败时静默吞 —— 这只是观测，写不进去最坏就是 UI
    // 上少了一行事件，不能拖垮主链路的 MCP 响应。tool_name 为空（caller 没传
    // name）也照写，便于事后发现 dispatcher 失败的样本。
    let (success, err_msg): (bool, Option<&str>) = match &result {
        Ok(_) => (true, None),
        Err(m) => (false, Some(m.as_str())),
    };
    let _ = db.insert_mcp_call(tool_name, latency_ms, success, err_msg);

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
    if let Some(obj) = value.as_object_mut()
        && let Some(id) = obj.get("id").and_then(|v| v.as_str())
    {
        let dl = deep_link_for_session(id);
        obj.insert("deep_link".to_string(), serde_json::Value::String(dl));
    }
}

fn enrich_search_result(value: &mut serde_json::Value) {
    if let Some(obj) = value.as_object_mut()
        && let Some(id) = obj.get("session_id").and_then(|v| v.as_str())
    {
        let dl = deep_link_for_session(id);
        obj.insert("deep_link".to_string(), serde_json::Value::String(dl));
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
        !memex_core::processor::privacy::is_private_session(&r.session_id, r.project.as_deref())
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
        !memex_core::processor::privacy::is_private_session(&s.id, s.project_path.as_deref())
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
    use memex_core::context::{ContextOptions, build_context, search_by_project};
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
        !memex_core::processor::privacy::is_private_session(&s.id, s.project_path.as_deref())
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
