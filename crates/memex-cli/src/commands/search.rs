use std::time::Instant;

use anyhow::Result;
use memex_core::memex_dir;
use memex_core::retriever::{Retriever, SearchFilter};
use memex_core::storage::db::Db;

#[allow(clippy::too_many_arguments)]
pub fn run(
    query: &str,
    limit: usize,
    json: bool,
    adapter: Option<String>,
    project: Option<String>,
    chunk_type: Option<String>,
    after: Option<String>,
    before: Option<String>,
) -> Result<()> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        if json {
            println!(
                "{}",
                serde_json::json!({"results": [], "error": "database not found, run `memex ingest` first"})
            );
        } else {
            eprintln!("Database not found. Run `memex ingest` first.");
        }
        return Ok(());
    }

    let db = Db::open(&db_path)?;
    let retriever = Retriever::new(&db);

    let filter = SearchFilter {
        adapter,
        project,
        session_id: None,
        chunk_type,
        after,
        before,
    };

    let start = Instant::now();
    let results = retriever.search_filtered(query, limit, &filter)?;
    let latency = start.elapsed().as_millis() as u64;

    let _ = db.write_access_log(query, results.len(), latency);
    let _ = db.record_search_latency(latency);

    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else if results.is_empty() {
        println!("No results for \"{}\"", query);
    } else {
        println!(
            "Found {} result(s) for \"{}\" ({} ms):\n",
            results.len(),
            query,
            latency
        );
        for (i, r) in results.iter().enumerate() {
            let session_prefix = &r.session_id[..8.min(r.session_id.len())];
            let src = r.adapter.as_deref().unwrap_or("?");
            println!(
                "{}. [{}] session:{} ({})",
                i + 1,
                r.chunk_type,
                session_prefix,
                src
            );
            println!("   {}", r.snippet.replace('\n', " "));
            println!("   reason: {}\n", r.match_reason);
        }
    }

    Ok(())
}
