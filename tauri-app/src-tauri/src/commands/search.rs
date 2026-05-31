use memex_core::memex_dir;
use memex_core::retriever::Retriever;
use memex_core::storage::db::Db;
use memex_core::storage::models::SearchResult;

#[tauri::command]
pub fn search_memex(query: String, limit: Option<usize>) -> Result<Vec<SearchResult>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let retriever = Retriever::new(&db);
    retriever
        .search(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}
