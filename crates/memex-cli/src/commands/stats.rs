use anyhow::Result;
use memex_core::memex_dir;
use memex_core::storage::db::Db;

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

    if json {
        println!(
            "{}",
            serde_json::json!({
                "sessions": sessions,
                "messages": messages,
                "chunks": chunks,
            })
        );
    } else {
        println!("Memex Statistics:");
        println!("  Sessions: {}", sessions);
        println!("  Messages: {}", messages);
        println!("  Chunks:   {}", chunks);
    }

    Ok(())
}
