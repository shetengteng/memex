use std::fs;

use anyhow::Result;
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::rebuild;

pub fn run(json: bool) -> Result<()> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");

    if db_path.exists() {
        let bak = memex.join("memex.db.bak");
        fs::copy(&db_path, &bak)?;
        fs::remove_file(&db_path)?;
        if !json {
            println!("Backed up existing database to memex.db.bak");
        }
    }

    let db = Db::open(&db_path)?;
    let stats = rebuild::rebuild_from_markdown(&memex, &db)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "sessions": stats.sessions,
                "messages": stats.messages,
                "chunks": stats.chunks,
                "errors": stats.errors,
            })
        );
    } else {
        println!("Rebuild complete:");
        println!("  Sessions: {}", stats.sessions);
        println!("  Messages: {}", stats.messages);
        println!("  Chunks:   {}", stats.chunks);
        if stats.errors > 0 {
            println!("  Errors:   {}", stats.errors);
        }
    }

    Ok(())
}
