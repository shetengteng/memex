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

    let provider = match crate::llm::select_provider_unified(db, &config.llm, memex_dir) {
        Some(p) => p,
        None => return,
    };

    try_l1_chunk_summaries(db, provider.as_ref());
    try_l2_session_summaries(db, provider.as_ref(), config.llm.summary_cooldown_secs);
    try_l3_project_summaries(db, provider.as_ref());
    try_l4_periodic_summaries(db, provider.as_ref());
    try_l4_weekly_summaries(db, provider.as_ref());
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
                    project_name: None,
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

fn try_l4_weekly_summaries(db: &Db, provider: &dyn crate::llm::provider::LlmProvider) {
    let _ = regenerate_weekly_report_inner(db, provider, /* force = */ false);
}

/// 强制重新生成当前 ISO 周的 L4 周报，无论数据库里是否已存在。
/// 失败时返回 Err；当前周没有任何可用 L2 会话摘要时返回 Ok(None)。
pub fn regenerate_weekly_report(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
) -> anyhow::Result<Option<crate::storage::db::AggregateSummaryRow>> {
    regenerate_weekly_report_inner(db, provider, /* force = */ true)
}

fn regenerate_weekly_report_inner(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
    force: bool,
) -> anyhow::Result<Option<crate::storage::db::AggregateSummaryRow>> {
    use chrono::{Datelike, Utc};
    let now = Utc::now();
    let iso = now.iso_week();
    let scope_key = format!("weekly:{}-W{:02}", iso.year(), iso.week());

    if !force
        && db
            .get_aggregate_summary("weekly", &scope_key)
            .ok()
            .flatten()
            .is_some()
    {
        return Ok(None);
    }

    let week_start = chrono::NaiveDate::from_isoywd_opt(iso.year(), iso.week(), chrono::Weekday::Mon)
        .map(|d| format!("{}T00:00:00+00:00", d));
    let week_end = chrono::NaiveDate::from_isoywd_opt(iso.year(), iso.week(), chrono::Weekday::Sun)
        .map(|d| format!("{}T23:59:59+00:00", d));
    let (after, before) = match (week_start, week_end) {
        (Some(a), Some(b)) => (a, b),
        _ => return Ok(None),
    };

    let min_sessions = if force { 1 } else { 3 };
    let sessions = match db.list_sessions_in_range(&after, &before) {
        Ok(s) if s.len() >= min_sessions => s,
        _ => return Ok(None),
    };

    let mut l2_summaries = Vec::new();
    for s in &sessions {
        if let Ok(Some(row)) = db.get_summary(&s.id, "L2_session") {
            l2_summaries.push(summarize::SessionSummary {
                title: row.title.unwrap_or_default(),
                summary: row.summary,
                topics: row.topics,
                decisions: row.decisions,
                project_name: None,
            });
        }
    }
    if l2_summaries.is_empty() {
        return Ok(None);
    }

    let period_label = format!("Week {}-W{:02}", iso.year(), iso.week());
    let fixed_title = format!("周报 {}-W{:02}", iso.year(), iso.week());
    let summary = summarize::summarize_period(provider, &period_label, &l2_summaries)
        .map_err(|e| {
            warn!("L4 weekly summarize failed: {}", e);
            anyhow::anyhow!(e)
        })?;
    db.upsert_aggregate_summary(
        "weekly",
        &scope_key,
        Some(&fixed_title),
        &summary.summary,
        &summary.topics,
        &summary.decisions,
        sessions.len() as i64,
    )?;
    Ok(db.get_aggregate_summary("weekly", &scope_key)?)
}

/// 根据 scope_key 重新生成指定的日报或周报。
/// scope_key 格式：`daily:2026-06-04` 或 `weekly:2026-W23`。
pub fn regenerate_report_by_key(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
    scope_key: &str,
) -> anyhow::Result<Option<crate::storage::db::AggregateSummaryRow>> {
    let (scope_type, date_part) = scope_key
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("invalid scope_key format: {}", scope_key))?;

    let (after, before, period_label, fixed_title) = match scope_type {
        "daily" => {
            let after = format!("{}T00:00:00+00:00", date_part);
            let before = format!("{}T23:59:59+00:00", date_part);
            let label = format!("Daily {}", date_part);
            let title = format!("日报 {}", date_part);
            (after, before, label, title)
        }
        "weekly" => {
            let (year, week) = parse_iso_week(date_part)?;
            let start = chrono::NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Mon)
                .ok_or_else(|| anyhow::anyhow!("invalid week: {}", date_part))?;
            let end = chrono::NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::Sun)
                .ok_or_else(|| anyhow::anyhow!("invalid week: {}", date_part))?;
            let after = format!("{}T00:00:00+00:00", start);
            let before = format!("{}T23:59:59+00:00", end);
            let label = format!("Week {}", date_part);
            let title = format!("周报 {}", date_part);
            (after, before, label, title)
        }
        other => return Err(anyhow::anyhow!("unsupported scope_type: {}", other)),
    };

    let sessions = db.list_sessions_in_range(&after, &before)?;
    info!("regenerate_report_by_key: scope_key={}, sessions={}", scope_key, sessions.len());
    if sessions.is_empty() {
        warn!("regenerate_report_by_key: no sessions in range {} .. {}", after, before);
        return Ok(None);
    }

    let mut l2_summaries = Vec::new();
    for s in &sessions {
        if let Ok(Some(row)) = db.get_summary(&s.id, "L2_session") {
            l2_summaries.push(summarize::SessionSummary {
                title: row.title.unwrap_or_default(),
                summary: row.summary,
                topics: row.topics,
                decisions: row.decisions,
                project_name: None,
            });
        }
    }
    info!("regenerate_report_by_key: l2_summaries={}", l2_summaries.len());
    if l2_summaries.is_empty() {
        warn!("regenerate_report_by_key: no L2 summaries found for {} sessions", sessions.len());
        return Ok(None);
    }

    let summary = summarize::summarize_period(provider, &period_label, &l2_summaries)
        .map_err(|e| {
            warn!("regenerate_report_by_key: LLM summarize_period failed: {}", e);
            anyhow::anyhow!(e)
        })?;
    info!("regenerate_report_by_key: generated summary_len={}, title='{}'", summary.summary.len(), fixed_title);
    db.upsert_aggregate_summary(
        scope_type,
        scope_key,
        Some(&fixed_title),
        &summary.summary,
        &summary.topics,
        &summary.decisions,
        sessions.len() as i64,
    )?;
    Ok(db.get_aggregate_summary(scope_type, scope_key)?)
}

fn parse_iso_week(s: &str) -> anyhow::Result<(i32, u32)> {
    // "2026-W23" → (2026, 23)
    let parts: Vec<&str> = s.split("-W").collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("invalid week format: {}", s));
    }
    let year: i32 = parts[0].parse()?;
    let week: u32 = parts[1].parse()?;
    Ok((year, week))
}

fn try_l4_periodic_summaries(db: &Db, provider: &dyn crate::llm::provider::LlmProvider) {
    let _ = regenerate_daily_report_inner(db, provider, /* force = */ false);
}

/// 强制重新生成今天的 L4 日报，无论数据库里是否已存在。
/// 失败时返回 Err；今天没有任何可用 L2 会话摘要时返回 Ok(None)。
pub fn regenerate_daily_report(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
) -> anyhow::Result<Option<crate::storage::db::AggregateSummaryRow>> {
    regenerate_daily_report_inner(db, provider, /* force = */ true)
}

fn regenerate_daily_report_inner(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
    force: bool,
) -> anyhow::Result<Option<crate::storage::db::AggregateSummaryRow>> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let scope_key = format!("daily:{}", today);

    if !force
        && db
            .get_aggregate_summary("daily", &scope_key)
            .ok()
            .flatten()
            .is_some()
    {
        return Ok(None);
    }

    let after = format!("{}T00:00:00+00:00", today);
    let before = format!("{}T23:59:59+00:00", today);
    let min_sessions = if force { 1 } else { 2 };
    let sessions = match db.list_sessions_in_range(&after, &before) {
        Ok(s) if s.len() >= min_sessions => s,
        _ => return Ok(None),
    };

    let mut l2_summaries = Vec::new();
    for s in &sessions {
        if let Ok(Some(row)) = db.get_summary(&s.id, "L2_session") {
            l2_summaries.push(summarize::SessionSummary {
                title: row.title.unwrap_or_default(),
                summary: row.summary,
                topics: row.topics,
                decisions: row.decisions,
                project_name: None,
            });
        }
    }
    if l2_summaries.is_empty() {
        return Ok(None);
    }

    let period_label = format!("Daily {}", today);
    let fixed_title = format!("日报 {}", today);
    let summary = summarize::summarize_period(provider, &period_label, &l2_summaries)
        .map_err(|e| {
            warn!("L4 daily summarize failed: {}", e);
            anyhow::anyhow!(e)
        })?;
    db.upsert_aggregate_summary(
        "daily",
        &scope_key,
        Some(&fixed_title),
        &summary.summary,
        &summary.topics,
        &summary.decisions,
        sessions.len() as i64,
    )?;
    Ok(db.get_aggregate_summary("daily", &scope_key)?)
}

fn try_l2_session_summaries(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
    cool_down_secs: u64,
) {
    // 同时使用方案 A（过期检测）+ 方案 B（会话冷却）：
    //   - 没有 L2 摘要的会话 → 新建；
    //   - session.message_count > 上次摘要时的快照 → 重生成；
    //   - 同时要求 sessions.updated_at 距今 ≥ cool_down_secs（默认 10 分钟），
    //     避免 ingest 高频抖动导致频繁重摘要。
    let session_ids = match db.sessions_needing_summary(20, cool_down_secs) {
        Ok(ids) => ids,
        Err(_) => return,
    };

    for session_id in session_ids {
        summarize_session_by_id(db, provider, &session_id);
    }
}

pub fn summarize_session_by_id(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
    session_id: &str,
) -> bool {
    let detail = match db.get_session_detail(session_id) {
        Ok(Some(d)) if d.messages.len() >= 2 => d,
        _ => return false,
    };

    // 关键：把「这次摘要覆盖了多少消息」记下来。下次 ingest 会比较
    // sessions.message_count（实际入库消息数）和这个快照：如果 session
    // 又涨了，就视为过期、重新摘要（方案 A）。
    let message_count_at_creation = detail.messages.len() as i64;

    let msgs: Vec<(String, String)> = detail
        .messages
        .iter()
        .map(|m| (m.role.clone(), m.content.clone()))
        .collect();

    match summarize::summarize_session(provider, &msgs) {
        Ok(summary) => {
            let _ = db.upsert_summary(
                session_id,
                "L2_session",
                Some(&summary.title),
                &summary.summary,
                &summary.topics,
                &summary.decisions,
                message_count_at_creation,
            );
            if let Some(ref name) = summary.project_name {
                let _ = db.update_session_project_path(session_id, name);
            }
            true
        }
        Err(e) => {
            warn!("L2 summarize failed for session {}: {}", &session_id[..8.min(session_id.len())], e);
            false
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

        // 永远先 upsert session 行，这样即便会话没有新消息，updated_at
        // 也能跟 adapter 的 mtime 同步刷新。
        // 这是后续 ingest 把历史时间戳逐步修正回真实活动时间的关键。
        db.insert_session_with_title(
            &session.id,
            adapter.name(),
            session.project_path.as_deref(),
            &session.file_path,
            session.created_secs,
            session.mtime,
            session.title.as_deref(),
        )?;

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
