use memex_core::memex_dir;
use memex_core::storage::db::{Db, SessionRow};

#[tauri::command]
pub fn list_recent(limit: Option<usize>) -> Result<Vec<SessionRow>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.list_sessions(limit.unwrap_or(10))
        .map_err(|e| e.to_string())
}
