use memex_core::memex_dir;
use memex_core::storage::db::{Db, SessionDetail, SessionRow};

#[tauri::command]
pub fn list_recent(limit: Option<usize>, offset: Option<usize>) -> Result<Vec<SessionRow>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.list_sessions_paged(limit.unwrap_or(20), offset.unwrap_or(0))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_session(session_id: String) -> Result<Option<SessionDetail>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(None);
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.get_session_detail(&session_id)
        .map_err(|e| e.to_string())
}
