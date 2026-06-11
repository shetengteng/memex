//! `memex sessions` —— Phase 5a 起走 daemon RPC。
//!
//! daemon 端 `/sessions?limit=N` 直接返回 `{sessions: [...]}`，没有 `days`
//! 过滤；这里在 client 拿到 JSON 之后做一次 `updated_at >= cutoff` 的筛。
//! 跨 sessions 命令的 days 用户接口保持不变。

use anyhow::Result;

use crate::client::MemexClient;

pub fn run(recent: usize, days: Option<u32>, json: bool) -> Result<()> {
    let client = MemexClient::connect()?;
    let body = client.get_value(&format!("/sessions?limit={}", recent))?;

    // daemon 端永远是 {sessions: [...]} 形状；如果上游变了，直接当空表处理。
    let Some(arr) = body.get("sessions").and_then(|v| v.as_array()) else {
        if json {
            crate::io::json(&serde_json::json!({"sessions": []}))?;
        } else {
            crate::out!("No sessions found.");
        }
        return Ok(());
    };

    // daemon 端 SessionEntry 用 `#[serde(rename_all = "camelCase")]`，所以这里
    // 用 camelCase 字段名取值；以前的 snake_case 是 db 模块的 Rust 字段名。
    let cutoff = days.map(|d| (chrono::Utc::now() - chrono::Duration::days(d as i64)).to_rfc3339());
    let sessions: Vec<&serde_json::Value> = arr
        .iter()
        .filter(|s| {
            let Some(updated) = s.get("updatedAt").and_then(|v| v.as_str()) else {
                return cutoff.is_none();
            };
            cutoff.as_deref().is_none_or(|c| updated >= c)
        })
        .collect();

    if json {
        crate::io::json(&serde_json::json!({ "sessions": sessions }))?;
        return Ok(());
    }
    if sessions.is_empty() {
        crate::out!("No sessions found.");
        return Ok(());
    }
    crate::out!("{} session(s):\n", sessions.len());
    for s in &sessions {
        let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let src = s.get("source").and_then(|v| v.as_str()).unwrap_or("?");
        let msgs = s.get("messageCount").and_then(|v| v.as_i64()).unwrap_or(0);
        let updated = s.get("updatedAt").and_then(|v| v.as_str()).unwrap_or("?");
        crate::out!(
            "  {} [{}] {} msgs  {}",
            &id[..8.min(id.len())],
            src,
            msgs,
            updated
        );
    }

    Ok(())
}
