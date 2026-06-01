use std::path::Path;

use anyhow::Result;
use tracing::{info, warn};

use crate::collector::{self, Adapter};
use crate::config::MemexConfig;
use crate::llm::summarize;
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

    let config = MemexConfig::load(memex_dir).unwrap_or_default();
    let adapters = collector::enabled_adapters(&config.adapters);
    let mut result = IngestResult::default();

    for adapter in &adapters {
        if adapter_filter.is_some_and(|f| f != adapter.name()) {
            continue;
        }
        match ingest_adapter(adapter.as_ref(), db, memex_dir) {
            Ok((msgs, chunks)) => {
                result.messages_ingested += msgs;
                result.chunks_created += chunks;
            }
            Err(e) => {
                tracing::warn!("{}: adapter error: {}", adapter.name(), e);
                let _ = db.increment_metric(crate::storage::metrics::METRIC_ADAPTER_ERRORS);
            }
        }
    }

    let _ = db.increment_metric(crate::storage::metrics::METRIC_INGEST_COUNT);
    if result.messages_ingested > 0 {
        let _ = db.increment_metric_by(
            crate::storage::metrics::METRIC_INGEST_MESSAGES,
            result.messages_ingested as i64,
        );
    }

    try_summarize_new_sessions(db, memex_dir);

    Ok(result)
}

fn try_summarize_new_sessions(db: &Db, memex_dir: &Path) {
    let config = match MemexConfig::load(memex_dir) {
        Ok(c) => c,
        Err(_) => return,
    };

    let provider = match crate::llm::select_provider(&config.llm, memex_dir) {
        Some(p) => p,
        None => return,
    };

    if provider.name() == "anthropic" {
        let shown = db
            .kv_get(crate::llm::CLOUD_NOTICE_KV_KEY)
            .ok()
            .flatten()
            .is_some();
        if !shown {
            warn!("{}", crate::llm::cloud_upload_scope());
            let _ = db.kv_set(crate::llm::CLOUD_NOTICE_KV_KEY, "true");
        }
    }

    try_l1_chunk_summaries(db, provider.as_ref());
    try_l2_session_summaries(db, provider.as_ref());
    try_l3_project_summaries(db, provider.as_ref());
    try_l4_periodic_summaries(db, provider.as_ref());
}

fn try_l1_chunk_summaries(db: &Db, provider: &dyn crate::llm::provider::LlmProvider) {
    let min_tokens = (summarize::min_chunk_chars() / 4) as u32;
    let chunks = match db.chunks_without_summary(min_tokens, summarize::l1_batch_size()) {
        Ok(c) => c,
        Err(_) => return,
    };

    for (chunk_id, content, _) in chunks {
        match summarize::summarize_chunk(provider, &content) {
            Ok(s) => {
                let _ = db.update_chunk_summary(chunk_id, &s);
            }
            Err(e) => {
                warn!("L1 summarize failed for chunk {}: {}", chunk_id, e);
            }
        }
    }
}

fn try_l3_project_summaries(db: &Db, provider: &dyn crate::llm::provider::LlmProvider) {
    let projects = match db.distinct_projects() {
        Ok(p) => p,
        Err(_) => return,
    };

    for project in projects {
        if db
            .get_aggregate_summary("project", &project)
            .ok()
            .flatten()
            .is_some()
        {
            continue;
        }

        let sessions = match db.list_sessions_by_project(&project) {
            Ok(s) if s.len() >= 2 => s,
            _ => continue,
        };

        let mut l2_summaries = Vec::new();
        for s in &sessions {
            if let Ok(Some(row)) = db.get_summary(&s.id, "L2_session") {
                l2_summaries.push(summarize::SessionSummary {
                    title: row.title.unwrap_or_default(),
                    summary: row.summary,
                    topics: row.topics,
                    decisions: row.decisions,
                });
            }
        }
        if l2_summaries.len() < 2 {
            continue;
        }

        match summarize::summarize_project(provider, &l2_summaries) {
            Ok(summary) => {
                let _ = db.upsert_aggregate_summary(
                    "project",
                    &project,
                    Some(&summary.title),
                    &summary.summary,
                    &summary.topics,
                    &summary.decisions,
                    sessions.len() as i64,
                );
            }
            Err(e) => {
                warn!("L3 project summarize failed for {}: {}", project, e);
            }
        }
    }
}

fn try_l4_periodic_summaries(db: &Db, provider: &dyn crate::llm::provider::LlmProvider) {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let scope_key = format!("daily:{}", today);

    if db
        .get_aggregate_summary("daily", &scope_key)
        .ok()
        .flatten()
        .is_some()
    {
        return;
    }

    let after = format!("{}T00:00:00+00:00", today);
    let before = format!("{}T23:59:59+00:00", today);
    let sessions = match db.list_sessions_in_range(&after, &before) {
        Ok(s) if s.len() >= 2 => s,
        _ => return,
    };

    let mut l2_summaries = Vec::new();
    for s in &sessions {
        if let Ok(Some(row)) = db.get_summary(&s.id, "L2_session") {
            l2_summaries.push(summarize::SessionSummary {
                title: row.title.unwrap_or_default(),
                summary: row.summary,
                topics: row.topics,
                decisions: row.decisions,
            });
        }
    }
    if l2_summaries.is_empty() {
        return;
    }

    let period_label = format!("Daily {}", today);
    match summarize::summarize_period(provider, &period_label, &l2_summaries) {
        Ok(summary) => {
            let _ = db.upsert_aggregate_summary(
                "daily",
                &scope_key,
                Some(&summary.title),
                &summary.summary,
                &summary.topics,
                &summary.decisions,
                sessions.len() as i64,
            );
        }
        Err(e) => {
            warn!("L4 daily summarize failed: {}", e);
        }
    }
}

fn try_l2_session_summaries(db: &Db, provider: &dyn crate::llm::provider::LlmProvider) {
    let sessions = match db.list_sessions(5) {
        Ok(s) => s,
        Err(_) => return,
    };

    for session in sessions {
        if db.get_summary(&session.id, "L2_session").ok().flatten().is_some() {
            continue;
        }
        let detail = match db.get_session_detail(&session.id) {
            Ok(Some(d)) if d.messages.len() >= 2 => d,
            _ => continue,
        };

        let msgs: Vec<(String, String)> = detail
            .messages
            .iter()
            .map(|m| (m.role.clone(), m.content.clone()))
            .collect();

        match summarize::summarize_session(provider, &msgs) {
            Ok(summary) => {
                let _ = db.upsert_summary(
                    &session.id,
                    "L2_session",
                    Some(&summary.title),
                    &summary.summary,
                    &summary.topics,
                    &summary.decisions,
                );
            }
            Err(e) => {
                warn!("L2 summarize failed for session {}: {}", &session.id[..8.min(session.id.len())], e);
            }
        }
    }
}

const MESSAGE_BATCH_SIZE: usize = 100;

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
            last_scan: chrono::Utc::now(),
        };
        db.upsert_source(&source_state)?;
    }

    Ok((msg_count, chunk_count))
}

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
                let _ = db.insert_redaction(
                    &pc.chunk.message_id,
                    &pc.chunk.session_id,
                    &hit.redaction_type,
                    hit.original_length,
                );
            }
        }
    }

    Ok((msg_count, chunk_count, max_offset))
}
