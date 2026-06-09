use anyhow::Result;
use memex_core::memex_dir;
use memex_core::storage::db::Db;

pub fn run(recent: usize, days: Option<u32>, json: bool) -> Result<()> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        if json {
            crate::io::json(&serde_json::json!({"sessions": []}))?;
        } else {
            crate::out!("No sessions found. Run `memex ingest` first.");
        }
        return Ok(());
    }

    let db = Db::open(&db_path)?;
    let mut sessions = db.list_sessions(recent)?;

    if let Some(d) = days {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(d as i64)).to_rfc3339();
        sessions.retain(|s| s.updated_at.as_str() >= cutoff.as_str());
    }

    if json {
        crate::io::json(&sessions)?;
    } else if sessions.is_empty() {
        crate::out!("No sessions found.");
    } else {
        crate::out!("{} session(s):\n", sessions.len());
        for s in &sessions {
            crate::out!(
                "  {} [{}] {} msgs  {}",
                &s.id[..8.min(s.id.len())],
                s.source,
                s.message_count,
                s.updated_at
            );
        }
    }

    Ok(())
}
