use anyhow::Result;
use memex_core::retriever::Retriever;
use memex_core::storage::db::Db;
use memex_core::memex_dir;

pub fn run(query: &str, limit: usize, json: bool) -> Result<()> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        if json {
            println!("{}", serde_json::json!({"results": [], "error": "database not found, run `memex ingest` first"}));
        } else {
            eprintln!("Database not found. Run `memex ingest` first.");
        }
        return Ok(());
    }

    let db = Db::open(&db_path)?;
    let retriever = Retriever::new(&db);
    let results = retriever.search(query, limit)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        if results.is_empty() {
            println!("No results for \"{}\"", query);
        } else {
            println!("Found {} result(s) for \"{}\":\n", results.len(), query);
            for (i, r) in results.iter().enumerate() {
                println!("{}. [{}] session:{}", i + 1, r.chunk_type, &r.session_id[..8.min(r.session_id.len())]);
                println!("   {}\n", r.snippet.replace('\n', " "));
            }
        }
    }

    Ok(())
}
