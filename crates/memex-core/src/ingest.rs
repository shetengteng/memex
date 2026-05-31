use std::path::Path;

use anyhow::Result;
use tracing::info;

use crate::collector::{self, Adapter};
use crate::processor;
use crate::storage::db::Db;
use crate::storage::markdown;
use crate::storage::models::SourceState;

#[derive(Debug, Clone, Default)]
pub struct IngestResult {
    pub messages_ingested: u64,
    pub chunks_created: u64,
}

pub fn run_ingest(db: &Db, memex_dir: &Path, adapter_filter: Option<&str>) -> Result<IngestResult> {
    let redactions_path = memex_dir.join("redactions.yaml");
    processor::redact::load_custom_rules(&redactions_path);
    processor::privacy::load_privacy_rules(&redactions_path);

    let adapters = collector::all_adapters();
    let mut result = IngestResult::default();

    for adapter in &adapters {
        if adapter_filter.is_some_and(|f| f != adapter.name()) {
            continue;
        }
        let (msgs, chunks) = ingest_adapter(adapter.as_ref(), db, memex_dir)?;
        result.messages_ingested += msgs;
        result.chunks_created += chunks;
    }

    let _ = db.increment_metric(crate::storage::metrics::METRIC_INGEST_COUNT);
    if result.messages_ingested > 0 {
        let _ = db.increment_metric_by(
            crate::storage::metrics::METRIC_INGEST_MESSAGES,
            result.messages_ingested as i64,
        );
    }

    Ok(result)
}

fn ingest_adapter(adapter: &dyn Adapter, db: &Db, memex: &Path) -> Result<(u64, u64)> {
    let sessions = adapter.scan()?;
    info!("{}: found {} session files", adapter.name(), sessions.len());

    let mut msg_count = 0u64;
    let mut chunk_count = 0u64;

    for mut session in sessions {
        if processor::privacy::is_private_session(
            &session.file_path,
            session.project_path.as_deref(),
        ) {
            info!(
                "{}: skipping private session {}",
                adapter.name(),
                &session.id[..8.min(session.id.len())]
            );
            continue;
        }

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
