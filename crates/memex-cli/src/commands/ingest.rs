use anyhow::Result;
use memex_core::collector::claude_code::ClaudeCodeAdapter;
use memex_core::collector::Adapter;
use memex_core::config::ensure_memex_dir;
use memex_core::processor;
use memex_core::storage::db::Db;
use memex_core::storage::markdown;
use memex_core::storage::models::SourceState;
use memex_core::{memex_dir};
use tracing::info;

pub fn run(adapter_filter: Option<&str>, json: bool) -> Result<()> {
    let memex = memex_dir();
    ensure_memex_dir(&memex)?;

    let db_path = memex.join("memex.db");
    let db = Db::open(&db_path)?;

    let mut total_messages = 0u64;
    let mut total_chunks = 0u64;

    if adapter_filter.is_none() || adapter_filter == Some("claude_code") {
        let (msgs, chunks) = ingest_adapter(&ClaudeCodeAdapter::new(), &db, &memex)?;
        total_messages += msgs;
        total_chunks += chunks;
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "messages_ingested": total_messages,
                "chunks_created": total_chunks,
            })
        );
    } else {
        println!("Ingested {} messages, created {} chunks", total_messages, total_chunks);
    }

    Ok(())
}

fn ingest_adapter(adapter: &dyn Adapter, db: &Db, memex: &std::path::Path) -> Result<(u64, u64)> {
    let sessions = adapter.scan()?;
    info!("{}: found {} session files", adapter.name(), sessions.len());

    let mut msg_count = 0u64;
    let mut chunk_count = 0u64;

    for mut session in sessions {
        let stored_offset = db.get_source_offset(&session.file_path)?;
        session.last_offset = stored_offset;

        let messages = adapter.collect(&session)?;
        if messages.is_empty() {
            continue;
        }

        db.insert_session(
            &session.id,
            adapter.name(),
            session.project_path.as_deref(),
            &session.file_path,
        )?;

        let mut max_offset = stored_offset;
        let mut inserted_messages = Vec::new();

        for msg in &messages {
            let content_hash = blake3::hash(msg.content.as_bytes()).to_hex().to_string();
            let ts_str = msg.timestamp.map(|t| t.to_rfc3339());
            let inserted = db.insert_message(
                &msg.id,
                &session.id,
                &msg.role.to_string(),
                &msg.content,
                ts_str.as_deref(),
                msg.source_offset,
                &content_hash,
            )?;

            if inserted {
                msg_count += 1;
                inserted_messages.push(msg);
                if msg.source_offset > max_offset {
                    max_offset = msg.source_offset;
                }
            }
        }

        if !inserted_messages.is_empty() {
            let owned: Vec<_> = inserted_messages.into_iter().cloned().collect();
            let chunks = processor::process_messages(&owned)?;
            for chunk in &chunks {
                db.insert_chunk(chunk)?;
                chunk_count += 1;
            }
        }

        markdown::write_session_markdown(
            memex,
            &session.id,
            adapter.name(),
            session.project_path.as_deref(),
            &messages,
        )?;

        let source_state = SourceState {
            adapter: adapter.name().to_string(),
            file_path: session.file_path.clone(),
            last_offset: max_offset,
            last_mtime: session.mtime,
            last_scan: chrono::Utc::now(),
        };
        db.upsert_source(&source_state)?;
    }

    Ok((msg_count, chunk_count))
}
