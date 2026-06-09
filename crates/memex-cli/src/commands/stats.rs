use anyhow::Result;
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::metrics::{
    METRIC_ADAPTER_ERRORS, METRIC_INGEST_COUNT, METRIC_INGEST_MESSAGES, METRIC_MCP_CALLS,
    METRIC_SEARCH_COUNT, METRIC_SLOW_QUERIES,
};

const ACTIVITY_WINDOW_DAYS: u32 = 7;

pub fn run(json: bool) -> Result<()> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        if json {
            println!(
                "{}",
                serde_json::json!({"sessions": 0, "messages": 0, "chunks": 0})
            );
        } else {
            println!("No data yet. Run `memex ingest` first.");
        }
        return Ok(());
    }

    let db = Db::open(&db_path)?;
    let sessions = db.session_count()?;
    let messages = db.message_count()?;
    let chunks = db.chunk_count()?;

    let today = db.get_today_metrics().unwrap_or_default();
    let last_week_total = aggregate_range(&db, ACTIVITY_WINDOW_DAYS);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "sessions": sessions,
                "messages": messages,
                "chunks": chunks,
                "today": metric_map(&today),
                "last_7_days": last_week_total,
            })
        );
        return Ok(());
    }

    println!("Memex Statistics:");
    println!("  Sessions: {}", sessions);
    println!("  Messages: {}", messages);
    println!("  Chunks:   {}", chunks);

    println!("\nToday:");
    print_activity_line("  Searches", &today, METRIC_SEARCH_COUNT);
    print_activity_line("  MCP calls", &today, METRIC_MCP_CALLS);
    print_activity_line("  Slow queries", &today, METRIC_SLOW_QUERIES);
    print_activity_line("  Ingest runs", &today, METRIC_INGEST_COUNT);
    print_activity_line("  Ingested msgs", &today, METRIC_INGEST_MESSAGES);
    print_activity_line("  Adapter errors", &today, METRIC_ADAPTER_ERRORS);

    println!("\nLast {} days:", ACTIVITY_WINDOW_DAYS);
    print_total_line("  Searches", &last_week_total, METRIC_SEARCH_COUNT);
    print_total_line("  MCP calls", &last_week_total, METRIC_MCP_CALLS);
    print_total_line("  Slow queries", &last_week_total, METRIC_SLOW_QUERIES);
    print_total_line("  Ingest runs", &last_week_total, METRIC_INGEST_COUNT);
    print_total_line("  Ingested msgs", &last_week_total, METRIC_INGEST_MESSAGES);
    print_total_line("  Adapter errors", &last_week_total, METRIC_ADAPTER_ERRORS);

    Ok(())
}

fn metric_map(entries: &[memex_core::storage::metrics::MetricEntry]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for e in entries {
        map.insert(e.name.clone(), serde_json::Value::from(e.value));
    }
    serde_json::Value::Object(map)
}

fn aggregate_range(db: &Db, days: u32) -> std::collections::BTreeMap<String, i64> {
    let mut totals: std::collections::BTreeMap<String, i64> = Default::default();
    let Ok(daily) = db.get_metrics_range(days) else {
        return totals;
    };
    for day in daily {
        for entry in day.entries {
            *totals.entry(entry.name).or_insert(0) += entry.value;
        }
    }
    totals
}

fn print_activity_line(
    label: &str,
    entries: &[memex_core::storage::metrics::MetricEntry],
    name: &str,
) {
    let value = entries
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.value)
        .unwrap_or(0);
    println!("{}: {}", label, value);
}

fn print_total_line(label: &str, totals: &std::collections::BTreeMap<String, i64>, name: &str) {
    let value = totals.get(name).copied().unwrap_or(0);
    println!("{}: {}", label, value);
}
