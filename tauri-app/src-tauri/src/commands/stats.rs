use memex_core::memex_dir;
use memex_core::storage::db::Db;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Stats {
    pub sessions: u64,
    pub messages: u64,
    pub chunks: u64,
    pub db_exists: bool,
}

#[tauri::command]
pub fn get_stats() -> Result<Stats, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(Stats {
            sessions: 0,
            messages: 0,
            chunks: 0,
            db_exists: false,
        });
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    Ok(Stats {
        sessions: db.session_count().unwrap_or(0),
        messages: db.message_count().unwrap_or(0),
        chunks: db.chunk_count().unwrap_or(0),
        db_exists: true,
    })
}
