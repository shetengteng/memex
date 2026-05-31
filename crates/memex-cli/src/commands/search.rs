use std::time::Instant;

use anyhow::Result;
use memex_core::memex_dir;
use memex_core::retriever::{Retriever, SearchFilter};
use memex_core::storage::db::Db;

use super::daemon_client;

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
    let memex = memex_dir();

    if let Some(port) = daemon_client::daemon_port(&memex) {
        return run_via_http(port, query, limit, json, &adapter, &project, &chunk_type, &after, &before);
    }

    run_direct(query, limit, json, adapter, project, chunk_type, after, before)
}

#[allow(clippy::too_many_arguments)]
fn run_via_http(
    port: u16,
    query: &str,
    limit: usize,
    json: bool,
    adapter: &Option<String>,
    project: &Option<String>,
    chunk_type: &Option<String>,
    after: &Option<String>,
    before: &Option<String>,
) -> Result<()> {
    let mut params = format!("/search?q={}&limit={}", urlenc(query), limit);
    if let Some(a) = adapter { params.push_str(&format!("&adapter={}", urlenc(a))); }
    if let Some(p) = project { params.push_str(&format!("&project={}", urlenc(p))); }
    if let Some(c) = chunk_type { params.push_str(&format!("&chunk_type={}", urlenc(c))); }
    if let Some(a) = after { params.push_str(&format!("&after={}", urlenc(a))); }
    if let Some(b) = before { params.push_str(&format!("&before={}", urlenc(b))); }

    let start = Instant::now();
    match daemon_client::http_get_json(port, &params) {
        Ok(body) => {
            let latency = start.elapsed().as_millis();
            if json {
                println!("{}", serde_json::to_string_pretty(&body)?);
            } else if let Some(results) = body.get("results").and_then(|v| v.as_array()) {
                if results.is_empty() {
                    println!("No results for \"{}\"", query);
                } else {
                    println!("Found {} result(s) for \"{}\" ({} ms, via daemon):\n", results.len(), query, latency);
                    for (i, r) in results.iter().enumerate() {
                        let sid = r.get("session_id").and_then(|v| v.as_str()).unwrap_or("?");
                        let prefix = &sid[..8.min(sid.len())];
                        let ct = r.get("chunk_type").and_then(|v| v.as_str()).unwrap_or("?");
                        let src = r.get("adapter").and_then(|v| v.as_str()).unwrap_or("?");
                        let snip = r.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
                        let reason = r.get("match_reason").and_then(|v| v.as_str()).unwrap_or("");
                        println!("{}. [{}] session:{} ({})", i + 1, ct, prefix, src);
                        println!("   {}", snip.replace('\n', " "));
                        println!("   reason: {}\n", reason);
                    }
                }
            }
            Ok(())
        }
        Err(_) => {
            run_direct(
                query, limit, json,
                adapter.clone(), project.clone(), chunk_type.clone(), after.clone(), before.clone(),
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_direct(
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
            println!("{}", serde_json::json!({"results": [], "error": "database not found, run `memex ingest` first"}));
        } else {
            eprintln!("Database not found. Run `memex ingest` first.");
        }
        return Ok(());
    }

    let db = Db::open(&db_path)?;
    let retriever = Retriever::new(&db);
    let filter = SearchFilter { adapter, project, session_id: None, chunk_type, after, before };

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
        println!("Found {} result(s) for \"{}\" ({} ms):\n", results.len(), query, latency);
        for (i, r) in results.iter().enumerate() {
            let prefix = &r.session_id[..8.min(r.session_id.len())];
            let src = r.adapter.as_deref().unwrap_or("?");
            println!("{}. [{}] session:{} ({})", i + 1, r.chunk_type, prefix, src);
            println!("   {}", r.snippet.replace('\n', " "));
            println!("   reason: {}\n", r.match_reason);
        }
    }

    Ok(())
}

fn urlenc(s: &str) -> String {
    s.replace(' ', "%20").replace('&', "%26").replace('=', "%3D")
}
