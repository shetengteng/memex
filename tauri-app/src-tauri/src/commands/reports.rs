use memex_core::memex_dir;
use memex_core::storage::db::{AggregateSummaryRow, Db};

#[tauri::command]
pub async fn list_reports(scope: String, limit: Option<u32>) -> Result<Vec<AggregateSummaryRow>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.list_aggregate_summaries(&scope, limit.unwrap_or(60))
        .map_err(|e| e.to_string())
}
