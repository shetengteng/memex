use anyhow::Result;
use memex_core::memex_dir;
use memex_core::storage::db::Db;

pub fn run(recent: usize, json: bool) -> Result<()> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        if json {
            println!("{}", serde_json::json!({"sessions": []}));
        } else {
            println!("No sessions found. Run `memex ingest` first.");
        }
        return Ok(());
    }

    let db = Db::open(&db_path)?;
    let sessions = db.list_sessions(recent)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
    } else {
        if sessions.is_empty() {
            println!("No sessions found.");
        } else {
            println!("{} session(s):\n", sessions.len());
            for s in &sessions {
                println!(
                    "  {} [{}] {} msgs  {}",
                    &s.id[..8.min(s.id.len())],
                    s.source,
                    s.message_count,
                    s.updated_at
                );
            }
        }
    }

    Ok(())
}
