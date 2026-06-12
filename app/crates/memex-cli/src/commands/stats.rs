//! `memex stats` —— Phase 5b-2 起完全走 daemon RPC。
//!
//! daemon 端 `/stats` 现在一次性返回 totals + today + last_7_days，CLI 只负责
//! 把 JSON 转成两段 human-readable 输出，跟 --json 模式下的原始 JSON 一致。

use anyhow::Result;

use crate::client::MemexClient;

const ACTIVITY_WINDOW_DAYS: u32 = 7;

/// 跟 daemon `routes::STATS_RANGE_DAYS` 字段名对齐，避免文案漂移。
const METRIC_LABELS: &[(&str, &str)] = &[
    ("search_count", "Searches"),
    ("mcp_calls", "MCP calls"),
    ("slow_queries", "Slow queries"),
    ("ingest_count", "Ingest runs"),
    ("ingest_messages", "Ingested msgs"),
    ("adapter_errors", "Adapter errors"),
];

pub fn run(json: bool) -> Result<()> {
    let client = MemexClient::connect()?;
    let body = client.get_value("/stats")?;

    if json {
        crate::io::json(&body)?;
        return Ok(());
    }

    let sessions = body.get("sessions").and_then(|v| v.as_i64()).unwrap_or(0);
    let messages = body.get("messages").and_then(|v| v.as_i64()).unwrap_or(0);
    let chunks = body.get("chunks").and_then(|v| v.as_i64()).unwrap_or(0);

    crate::out!("Memex Statistics:");
    crate::out!("  Sessions: {}", sessions);
    crate::out!("  Messages: {}", messages);
    crate::out!("  Chunks:   {}", chunks);

    let today = body.get("today");
    crate::out!("\nToday:");
    for (key, label) in METRIC_LABELS {
        let value = today.and_then(|t| t.get(*key)).and_then(|v| v.as_i64()).unwrap_or(0);
        crate::out!("  {}: {}", label, value);
    }

    let week = body.get("last_7_days");
    crate::out!("\nLast {} days:", ACTIVITY_WINDOW_DAYS);
    for (key, label) in METRIC_LABELS {
        let value = week.and_then(|w| w.get(*key)).and_then(|v| v.as_i64()).unwrap_or(0);
        crate::out!("  {}: {}", label, value);
    }

    Ok(())
}
