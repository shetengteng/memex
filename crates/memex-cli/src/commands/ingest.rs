use anyhow::Result;
use memex_core::config::ensure_memex_dir;
use memex_core::ingest;
use memex_core::memex_dir;
use memex_core::storage::db::Db;

pub fn run(adapter_filter: Option<&str>, json: bool) -> Result<()> {
    let memex = memex_dir();
    ensure_memex_dir(&memex)?;

    let db_path = memex.join("memex.db");
    let db = Db::open(&db_path)?;

    let result = ingest::run_ingest(&db, &memex, adapter_filter)?;

    if json {
        crate::io::json(&serde_json::json!({
            "messages_ingested": result.messages_ingested,
            "chunks_created": result.chunks_created,
        }))?;
    } else {
        crate::out!(
            "Ingested {} messages, created {} chunks",
            result.messages_ingested,
            result.chunks_created
        );
    }

    Ok(())
}
