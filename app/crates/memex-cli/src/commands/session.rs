//! `memex session <id>` —— Phase 5a 起走 daemon RPC。
//!
//! `daemon` 端 `/sessions/{id}` 只接全 36 字符 UUID；CLI 用户习惯用 8 字符
//! 前缀（搜索结果里 print 的就是 8 字符）。本命令先按长度分流：
//! * `id.len() == 36` → 直接走 GET /sessions/{id}
//! * 否则 → 先 GET /sessions?limit=200 拿一批最近会话，client-side 按前缀匹配；
//!   命中即换全 id 再去取详情。这跟以前 `db.find_session_by_prefix` 的语义一致。

use anyhow::Result;

use crate::client::MemexClient;

const PREFIX_SEARCH_LIMIT: usize = 200;

pub fn run(session_id: &str, json: bool) -> Result<()> {
    let client = MemexClient::connect()?;

    let resolved = resolve_session_id(&client, session_id)?;
    let body = client.get_value(&format!("/sessions/{}", resolved))?;

    // daemon 端 404 会返回 {"error": "session not found"}（status 也是 404，
    // 但 ureq 默认 4xx/5xx 抛错），所以走到这里基本不会拿到 error；保留一手保护。
    if body.get("error").is_some() {
        if json {
            crate::io::json(&body)?;
        } else {
            crate::err!("Session \"{}\" not found.", session_id);
        }
        return Ok(());
    }

    if json {
        crate::io::json(&body)?;
        return Ok(());
    }

    // daemon 端 SessionDetail 同样用 camelCase 序列化（rename_all = "camelCase"）。
    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("?");
    let source = body.get("source").and_then(|v| v.as_str()).unwrap_or("?");
    let project = body.get("projectPath").and_then(|v| v.as_str());
    let created = body.get("createdAt").and_then(|v| v.as_str()).unwrap_or("?");
    let updated = body.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("?");
    let count = body
        .get("messageCount")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    crate::out!("Session: {}", id);
    crate::out!("Source:  {}", source);
    if let Some(p) = project {
        crate::out!("Project: {}", p);
    }
    crate::out!("Created: {}", created);
    crate::out!("Updated: {}", updated);
    crate::out!("Messages: {}\n", count);

    let Some(messages) = body.get("messages").and_then(|v| v.as_array()) else {
        return Ok(());
    };
    for (i, msg) in messages.iter().enumerate() {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("?");
        let role_icon = match role {
            "user" => "User",
            "assistant" => "Assistant",
            "system" => "System",
            "tool" => "Tool",
            other => other,
        };
        let ts = msg.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
        let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
        crate::out!("--- Message {} ({}) {} ---", i + 1, role_icon, ts);
        let preview = if content.len() > 500 {
            format!("{}...", &content[..500])
        } else {
            content.to_string()
        };
        crate::out!("{}\n", preview);
    }
    Ok(())
}

/// 把用户输入的可能前缀解析成完整的 36 字符 session id。
///
/// 行为兼容旧 `db.find_session_by_prefix`：
/// * 完整 id（长度 36）→ 原样返回
/// * 短前缀 → 拉最近 200 个 session，找第一个 `id.starts_with(prefix)` 的。
///   没命中就返回原前缀，让 daemon 端 GET /sessions/{prefix} 抛 404
///   保留"用户输入的 id 不存在"这条 user-facing 错误。
fn resolve_session_id(client: &MemexClient, input: &str) -> Result<String> {
    if input.len() == 36 {
        return Ok(input.to_string());
    }
    let body = client.get_value(&format!("/sessions?limit={}", PREFIX_SEARCH_LIMIT))?;
    let Some(arr) = body.get("sessions").and_then(|v| v.as_array()) else {
        return Ok(input.to_string());
    };
    for s in arr {
        if let Some(id) = s.get("id").and_then(|v| v.as_str())
            && id.starts_with(input)
        {
            return Ok(id.to_string());
        }
    }
    Ok(input.to_string())
}
