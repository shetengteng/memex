use memex_core::memex_dir;
use memex_core::storage::db::Db;

fn open_db() -> Result<Db, String> {
    let db_path = memex_dir().join("memex.db");
    Db::open(&db_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_config(key: String) -> Result<Option<String>, String> {
    let db = open_db()?;
    db.kv_get(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_config(key: String, value: String) -> Result<(), String> {
    let db = open_db()?;
    db.kv_set(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_adapter(adapter: String, enabled: bool) -> Result<(), String> {
    let db = open_db()?;
    let key = format!("adapter.{}.enabled", adapter);
    let val = if enabled { "true" } else { "false" };
    db.kv_set(&key, val).map_err(|e| e.to_string())
}
