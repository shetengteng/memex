use memex_core::memex_dir;
use memex_core::retriever::Retriever;
use memex_core::storage::db::Db;
use memex_core::storage::models::SearchResult;

use super::error::CmdResult;

#[tauri::command]
pub async fn search_memex(
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
) -> CmdResult<Vec<SearchResult>> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let db = Db::open(&db_path)?;
    let retriever = Retriever::new(&db);
    let real_limit = limit.unwrap_or(20);
    let real_offset = offset.unwrap_or(0);
    let fetch_limit = real_limit + real_offset;
    let mut results = retriever.search(&query, fetch_limit)?;
    if real_offset > 0 && real_offset < results.len() {
        results = results.split_off(real_offset);
    } else if real_offset >= results.len() {
        results.clear();
    }
    results.truncate(real_limit);
    Ok(results)
}
