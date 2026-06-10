//! Per-adapter ingest loop: scan source files, upsert sessions, dedup
//! messages, persist chunks + markdown.
//!
//! Called from [`super::run_ingest`] for every enabled
//! [`crate::collector::Adapter`].

use std::path::Path;

use anyhow::Result;
use tracing::info;

use crate::collector::Adapter;
use crate::processor;
use crate::storage::db::Db;
use crate::storage::markdown;
use crate::storage::models::SourceState;

#[cfg(test)]
mod tests;

/// Cap per-call message batch size. Keeps a single jsonl with tens of
/// thousands of messages from monopolizing the DB write lock and lets
/// the caller report progress between batches.
const MESSAGE_BATCH_SIZE: usize = 100;

/// Run one ingest pass for `adapter`. Returns `(messages_ingested,
/// chunks_created)` aggregated across every session the adapter
/// surfaced.
pub(super) fn ingest_adapter(adapter: &dyn Adapter, db: &Db, memex: &Path) -> Result<(u64, u64)> {
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

        // 永远先 upsert session 行，这样即便会话没有新消息，updated_at
        // 也能跟 adapter 的 mtime 同步刷新。
        // 这是后续 ingest 把历史时间戳逐步修正回真实活动时间的关键。
        db.insert_session_with_title(crate::storage::db::NewSession {
            id: &session.id,
            source: adapter.name(),
            project_path: session.project_path.as_deref(),
            file_path: &session.file_path,
            session_created_secs: session.created_secs,
            session_mtime_secs: session.mtime,
            title: session.title.as_deref(),
        })?;

        if messages.is_empty() {
            continue;
        }

        let needs_batching = messages.len() > MESSAGE_BATCH_SIZE;
        if needs_batching {
            info!(
                "{}: large session {} ({} messages), processing in batches of {}",
                adapter.name(),
                &session.id[..8.min(session.id.len())],
                messages.len(),
                MESSAGE_BATCH_SIZE,
            );
        }

        let mut max_offset = stored_offset;
        for batch in messages.chunks(MESSAGE_BATCH_SIZE) {
            let (batch_msgs, batch_chunks, batch_max) =
                ingest_message_batch(batch, &session.id, db, stored_offset)?;
            msg_count += batch_msgs;
            chunk_count += batch_chunks;
            if batch_max > max_offset {
                max_offset = batch_max;
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
            last_scan: db.now_utc(),
        };
        db.upsert_source(&source_state)?;
    }

    Ok((msg_count, chunk_count))
}

/// Persist one batch of raw messages: dedup by content hash, write
/// chunks + redaction hits, return per-batch `(messages, chunks,
/// max_offset)`.
fn ingest_message_batch(
    batch: &[crate::storage::models::RawMessage],
    session_id: &str,
    db: &Db,
    stored_offset: u64,
) -> Result<(u64, u64, u64)> {
    let mut msg_count = 0u64;
    let mut chunk_count = 0u64;
    let mut max_offset = stored_offset;
    let mut inserted_messages = Vec::new();

    for msg in batch {
        let content_hash = blake3::hash(msg.content.as_bytes()).to_hex().to_string();
        let ts_str = msg.timestamp.map(|t| t.to_rfc3339());
        let inserted = db.insert_message(
            &msg.id,
            session_id,
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
        let processed = processor::process_messages_with_hits(&owned)?;
        for pc in &processed {
            db.insert_chunk(&pc.chunk)?;
            chunk_count += 1;
            for hit in &pc.redaction_hits {
                if let Err(e) = db.insert_redaction(
                    &pc.chunk.message_id,
                    &pc.chunk.session_id,
                    &hit.redaction_type,
                    hit.original_length,
                ) {
                    tracing::warn!(
                        message_id = %pc.chunk.message_id,
                        redaction_type = %hit.redaction_type,
                        error = %e,
                        "failed to record redaction hit",
                    );
                }
            }
        }
    }

    Ok((msg_count, chunk_count, max_offset))
}
