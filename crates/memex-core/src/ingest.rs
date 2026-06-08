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

    let throttle_ms = config.llm.summarize_interval_ms;

    // L1（每个 chunk 一次 LLM 调用）和 L2（每个 session 一次 LLM 调用）
    // 都是"短时间高频"路径——在大库上自动 ingest 一次会触发几十次 HTTP 调用。
    // 不节流的话本地 Ollama 会被压到 100% GPU/CPU、风扇拉满、UI 卡顿。
    // 与手动 batch_summarize 共用同一个配置 `llm.summarize_interval_ms`，
    // 默认 2000ms（在 LlmConfig::default 里设置）。
    try_l1_chunk_summaries(db, provider.as_ref(), throttle_ms);
    try_l2_session_summaries(db, provider.as_ref(), config.llm.summary_cooldown_secs, throttle_ms);

    // L3 / L4 都是「整库一次性聚合」，每个 scope 只跑一次 LLM，
    // 不构成"短时间高频"压力，不需要节流。
    try_l3_project_summaries(db, provider.as_ref());
    try_l4_periodic_summaries(db, provider.as_ref());
    try_l4_weekly_summaries(db, provider.as_ref());
    try_l4_monthly_summaries(db, provider.as_ref());
}

fn try_l1_chunk_summaries(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
    throttle_ms: u64,
) {
    let min_tokens = (summarize::min_chunk_chars() / 4) as u32;
    let chunks = match db.chunks_without_summary(min_tokens, summarize::l1_batch_size()) {
        Ok(c) => c,
        Err(_) => return,
    };

    let total = chunks.len();
    for (i, (chunk_id, content, _)) in chunks.into_iter().enumerate() {
        match summarize::summarize_chunk(provider, &content) {
            Ok(s) => {
                let _ = db.update_chunk_summary(chunk_id, &s);
            }
            Err(e) => {
                warn!("L1 summarize failed for chunk {}: {}", chunk_id, e);
            }
        }

        // 节流：除最后一条外，每次 LLM 调用后 sleep 配置的 throttle 时长。
        // 这避免在大库上自动 ingest 一口气发出几十次 LLM 请求，把本地 Ollama
        // 压到 GPU/CPU 100%、风扇拉满、UI 卡顿的尴尬场景。
        let is_last = i + 1 == total;
        if !is_last && throttle_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(throttle_ms));
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
                    intent: None,
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

fn try_l4_monthly_summaries(db: &Db, provider: &dyn crate::llm::provider::LlmProvider) {
    let _ = regenerate_monthly_report_inner(db, provider, /* force = */ false);
}

/// 强制重新生成当前自然月（YYYY-MM）的 L4 月报，覆盖现有记录。
/// 当月没有可用 L2 摘要时返回 Ok(None)。
pub fn regenerate_monthly_report(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
) -> anyhow::Result<Option<crate::storage::db::AggregateSummaryRow>> {
    regenerate_monthly_report_inner(db, provider, /* force = */ true)
}

fn regenerate_monthly_report_inner(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
    force: bool,
) -> anyhow::Result<Option<crate::storage::db::AggregateSummaryRow>> {
    use chrono::Datelike;
    let today = chrono::Utc::now().date_naive();
    let (year, month) = (today.year(), today.month());
    let scope_key = format!("monthly:{:04}-{:02}", year, month);

    if !force
        && db
            .get_aggregate_summary("monthly", &scope_key)
            .ok()
            .flatten()
            .is_some()
    {
        return Ok(None);
    }

    let (after, before) = match month_range(year, month) {
        Some(r) => r,
        None => return Ok(None),
    };

    // 月报相比周报数据量更大，门槛设到 5 个会话；force 模式（用户主动点）下放到 1
    let min_sessions = if force { 1 } else { 5 };
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
                intent: None,
            });
        }
    }
    if l2_summaries.is_empty() {
        return Ok(None);
    }

    let period_label = format!("Monthly {:04}-{:02}", year, month);
    let fixed_title = format!("月报 {:04}-{:02}", year, month);
    let summary = summarize::summarize_period(provider, &period_label, &l2_summaries)
        .map_err(|e| {
            warn!("L4 monthly summarize failed: {}", e);
            anyhow::anyhow!(e)
        })?;
    db.upsert_aggregate_summary(
        "monthly",
        &scope_key,
        Some(&fixed_title),
        &summary.summary,
        &summary.topics,
        &summary.decisions,
        sessions.len() as i64,
    )?;
    Ok(db.get_aggregate_summary("monthly", &scope_key)?)
}

/// 返回 `year-month` 自然月的 ISO 8601 起止区间字符串
/// （`[YYYY-MM-01T00:00:00+00:00, YYYY-(M+1)-01T00:00:00+00:00)`）。
/// 12 月时下个月跨年到 (year+1)-01。
fn month_range(year: i32, month: u32) -> Option<(String, String)> {
    use chrono::NaiveDate;
    let start = NaiveDate::from_ymd_opt(year, month, 1)?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = NaiveDate::from_ymd_opt(next_year, next_month, 1)?;
    Some((
        format!("{}T00:00:00+00:00", start),
        format!("{}T00:00:00+00:00", end),
    ))
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
                intent: None,
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
        "monthly" => {
            let (year, month) = parse_year_month(date_part)?;
            let (after, before) = month_range(year, month)
                .ok_or_else(|| anyhow::anyhow!("invalid month: {}", date_part))?;
            let label = format!("Monthly {}", date_part);
            let title = format!("月报 {}", date_part);
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
                intent: None,
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

fn parse_year_month(s: &str) -> anyhow::Result<(i32, u32)> {
    // "2026-06" → (2026, 6)
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("invalid month format: {}", s));
    }
    let year: i32 = parts[0].parse()?;
    let month: u32 = parts[1].parse()?;
    if !(1..=12).contains(&month) {
        return Err(anyhow::anyhow!("month out of range: {}", month));
    }
    Ok((year, month))
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
                intent: None,
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
    throttle_ms: u64,
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

    let total = session_ids.len();
    for (i, session_id) in session_ids.iter().enumerate() {
        summarize_session_by_id(db, provider, session_id);

        // 节流：每次 LLM 调用后 sleep（最后一条除外）。
        // 与 batch_summarize 共用 llm.summarize_interval_ms 配置。
        // throttle_ms = 0 时退化为旧行为（不 sleep）。
        let is_last = i + 1 == total;
        if !is_last && throttle_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(throttle_ms));
        }
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
            // 每次重生成都覆盖 sessions.intent，None 时显式写入 NULL，
            // 避免「重新生成后旧 intent 文本继续留在 UI 上」的脏数据。
            let _ = db.update_session_intent(session_id, summary.intent.as_deref());
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

/// L5「主题线索」聚类：手动触发。
///
/// 从最近的 80 个有 L2 摘要的 session 出发，喂给 LLM 让它聚类成 thread。
/// 没有可用 LLM 时退回到按 topic 分桶的 fallback —— 保证 UI 一定有内容看。
///
/// 返回成功落库的 thread 数。
///
/// 注意：此函数**不进入 try_summarize_new_sessions** 自动链路。
/// 聚类一次涉及一个大 prompt（80 条摘要 × 200 字 ≈ 16k token 输入），
/// 自动触发会让本地 Ollama 在每次 ingest 完成时都跑一遍，得不偿失。
/// 用户在 Library / 设置里手动点击「重新聚类」时触发。
pub fn regenerate_threads(
    db: &Db,
    provider: &dyn crate::llm::provider::LlmProvider,
) -> anyhow::Result<usize> {
    // 1) 取最近 80 个 session 的 L2 摘要
    let sessions = db.list_sessions_paged(80, 0)?;
    if sessions.is_empty() {
        return Ok(0);
    }

    let mut batch = Vec::with_capacity(sessions.len());
    for s in &sessions {
        let row = match db.get_summary(&s.id, "L2_session") {
            Ok(Some(r)) => r,
            _ => continue,
        };
        batch.push((
            s.id.clone(),
            summarize::SessionSummary {
                title: row.title.unwrap_or_default(),
                summary: row.summary,
                topics: row.topics,
                decisions: row.decisions,
                project_name: None,
                intent: None,
            },
        ));
    }
    if batch.is_empty() {
        return Ok(0);
    }

    // 2) LLM 聚类；失败用 fallback
    let drafts = match crate::llm::threads::cluster_threads(provider, &batch) {
        Ok(d) if !d.is_empty() => d,
        Ok(_) => {
            warn!("L5 LLM 聚类返回 0 个 thread，使用 fallback");
            crate::llm::threads::fallback_cluster(&batch)
        }
        Err(e) => {
            warn!("L5 LLM 聚类失败，使用 fallback: {}", e);
            crate::llm::threads::fallback_cluster(&batch)
        }
    };

    // 3) 落库（upsert：同名 thread 会覆盖 link，避免漂移）
    let mut ok = 0usize;
    for d in &drafts {
        match db.upsert_thread_with_sessions(d) {
            Ok(_) => ok += 1,
            Err(e) => warn!("upsert thread '{}' failed: {}", d.name, e),
        }
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_year_month_valid_inputs() {
        assert_eq!(parse_year_month("2026-06").unwrap(), (2026, 6));
        assert_eq!(parse_year_month("2025-01").unwrap(), (2025, 1));
        assert_eq!(parse_year_month("2025-12").unwrap(), (2025, 12));
    }

    #[test]
    fn parse_year_month_rejects_bad_format() {
        assert!(parse_year_month("2026").is_err());
        assert!(parse_year_month("2026-13").is_err(), "13 月不应通过");
        assert!(parse_year_month("2026-00").is_err(), "0 月不应通过");
        assert!(parse_year_month("abcd-06").is_err());
    }

    #[test]
    fn month_range_normal_month() {
        let (a, b) = month_range(2026, 6).unwrap();
        assert_eq!(a, "2026-06-01T00:00:00+00:00");
        assert_eq!(b, "2026-07-01T00:00:00+00:00");
    }

    #[test]
    fn month_range_december_crosses_year() {
        let (a, b) = month_range(2026, 12).unwrap();
        assert_eq!(a, "2026-12-01T00:00:00+00:00");
        assert_eq!(b, "2027-01-01T00:00:00+00:00");
    }

    #[test]
    fn month_range_february_is_normal_28_or_29() {
        // 2026 不是闰年，2 月 1 → 3 月 1
        let (a, b) = month_range(2026, 2).unwrap();
        assert_eq!(a, "2026-02-01T00:00:00+00:00");
        assert_eq!(b, "2026-03-01T00:00:00+00:00");
    }

    /// 节流（throttle）回归：
    /// 自动模式的 `try_l1_chunk_summaries` 和 `try_l2_session_summaries` 应该
    /// 在每两次 LLM 调用之间 sleep `llm.summarize_interval_ms`。
    /// 我们用一个会记录调用时刻的 mock provider 验证间隔确实 ≥ throttle。
    ///
    /// 为了让测试跑得快，throttle 设 50ms，调用 3 次 → 至少 100ms 间隔。
    /// 真实运行时配置为 2000ms，行为相同。
    #[test]
    fn throttle_inserts_sleep_between_l1_chunk_summaries() {
        use crate::llm::provider::{LlmProvider, LlmRequest, LlmResponse};
        use crate::storage::db::Db;
        use crate::storage::models::{Chunk, ChunkMetadata, ChunkType};
        use std::sync::Mutex;
        use std::time::Instant;

        struct TickProvider {
            ticks: Mutex<Vec<Instant>>,
        }
        impl LlmProvider for TickProvider {
            fn name(&self) -> &str { "tick" }
            fn is_available(&self) -> bool { true }
            fn generate(&self, _req: &LlmRequest) -> anyhow::Result<LlmResponse> {
                self.ticks.lock().unwrap().push(Instant::now());
                Ok(LlmResponse {
                    text: "summary".into(),
                    model: "tick".into(),
                    tokens_used: 1,
                })
            }
        }

        let db = Db::open_in_memory().unwrap();
        db.insert_session("s1", "claude_code", Some("/p"), "/f.jsonl", 0, 0).unwrap();
        // 插 3 条 chunk，每条都有足够大的 token_count 触发 summarize（min 阈值是
        // min_chunk_chars / 4 ≈ 50/4 ≈ 12 → 我们给 50）。
        for i in 0..3 {
            db.insert_message(
                &format!("m{i}"),
                "s1",
                "user",
                &format!("message {i} with enough content to summarize lalalalala"),
                None,
                i,
                &blake3::hash(format!("msg{i}").as_bytes()).to_hex().to_string(),
            ).unwrap();
            // 内容必须 ≥ 200 字符（MIN_CHUNK_CHARS_FOR_SUMMARY），否则
            // summarize_chunk 走 fallback 路径不调用 provider.generate。
            let long_content = "x".repeat(220);
            db.insert_chunk(&Chunk {
                id: None,
                message_id: format!("m{i}"),
                session_id: "s1".into(),
                chunk_type: ChunkType::Text,
                content: long_content,
                redacted_content: None,
                position: i as u32,
                token_count: 60,
                metadata: ChunkMetadata::default(),
            }).unwrap();
        }

        let provider = TickProvider { ticks: Mutex::new(Vec::new()) };

        // throttle=50ms 跑 3 个 chunk → 至少 2 个间隔，间隔 ≥ 50ms
        let start = Instant::now();
        try_l1_chunk_summaries(&db, &provider, /* throttle_ms = */ 50);
        let elapsed = start.elapsed();

        let ticks = provider.ticks.lock().unwrap();
        assert_eq!(ticks.len(), 3, "应该跑了 3 次 LLM 调用");

        // 间隔 0→1, 1→2 都应该 ≥ 50ms
        for i in 1..ticks.len() {
            let gap = ticks[i].duration_since(ticks[i - 1]);
            assert!(
                gap.as_millis() >= 45, // 留一点点容差，主要确认 sleep 真的发生
                "第 {} 次和第 {} 次 LLM 调用间隔应该 ≥ 50ms，实际 {:?}",
                i - 1, i, gap
            );
        }
        // 整体至少要 100ms（两个间隔）
        assert!(
            elapsed.as_millis() >= 90,
            "3 次调用应该至少 100ms，实际 {:?}",
            elapsed
        );
    }

    /// 节流 = 0 时退化为旧行为（不 sleep），确保我们不破坏现有用户配置。
    #[test]
    fn throttle_zero_disables_sleep() {
        use crate::llm::provider::{LlmProvider, LlmRequest, LlmResponse};
        use crate::storage::db::Db;
        use crate::storage::models::{Chunk, ChunkMetadata, ChunkType};
        use std::sync::Mutex;
        use std::time::Instant;

        struct FastProvider {
            count: Mutex<usize>,
        }
        impl LlmProvider for FastProvider {
            fn name(&self) -> &str { "fast" }
            fn is_available(&self) -> bool { true }
            fn generate(&self, _req: &LlmRequest) -> anyhow::Result<LlmResponse> {
                *self.count.lock().unwrap() += 1;
                Ok(LlmResponse {
                    text: "s".into(),
                    model: "fast".into(),
                    tokens_used: 1,
                })
            }
        }

        let db = Db::open_in_memory().unwrap();
        db.insert_session("s1", "claude_code", Some("/p"), "/f.jsonl", 0, 0).unwrap();
        for i in 0..5 {
            db.insert_message(
                &format!("m{i}"),
                "s1", "user",
                &format!("hello {i}"),
                None, i,
                &blake3::hash(format!("h{i}").as_bytes()).to_hex().to_string(),
            ).unwrap();
            // 见上文：内容必须 ≥ 200 字符才会调用 provider
            let long_content = "y".repeat(220);
            db.insert_chunk(&Chunk {
                id: None,
                message_id: format!("m{i}"),
                session_id: "s1".into(),
                chunk_type: ChunkType::Text,
                content: long_content,
                redacted_content: None,
                position: i as u32,
                token_count: 60,
                metadata: ChunkMetadata::default(),
            }).unwrap();
        }

        let provider = FastProvider { count: Mutex::new(0) };

        let start = Instant::now();
        try_l1_chunk_summaries(&db, &provider, /* throttle_ms = */ 0);
        let elapsed = start.elapsed();

        assert_eq!(*provider.count.lock().unwrap(), 5, "应跑 5 个 chunk");
        // 5 次内存调用（无 sleep）应该远低于 100ms
        assert!(
            elapsed.as_millis() < 200,
            "throttle=0 时 5 次调用不应该花太久，实际 {:?}",
            elapsed
        );
    }
}
