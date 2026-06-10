//! L4 periodic aggregation: daily / weekly / monthly reports plus the
//! generic "regenerate by scope_key" entry point. All four reports
//! share the same shape (collect L2 summaries in a date range, ask
//! the LLM to combine them, upsert an [`AggregateSummaryRow`]); the
//! differences are confined to a [`ReportSpec`] each entry point
//! builds for [`run_report`].

use anyhow::{Result, anyhow};
use chrono::{Datelike, NaiveDate, Utc, Weekday};
use tracing::{info, warn};

use super::period::{month_range, parse_iso_week, parse_year_month};
use crate::llm::provider::LlmProvider;
use crate::llm::summarize;
use crate::storage::db::{AggregateSummaryRow, AggregateSummaryUpsert, Db};

// ---- Auto-triggered (from `try_summarize_new_sessions`) -------------

pub(super) fn try_l4_weekly_summaries(db: &Db, provider: &dyn LlmProvider) {
    if let Err(e) = regenerate_weekly_report_inner(db, provider, /* force = */ false) {
        warn!(error = %e, "auto L4 weekly report generation failed");
    }
}

pub(super) fn try_l4_monthly_summaries(db: &Db, provider: &dyn LlmProvider) {
    if let Err(e) = regenerate_monthly_report_inner(db, provider, /* force = */ false) {
        warn!(error = %e, "auto L4 monthly report generation failed");
    }
}

pub(super) fn try_l4_periodic_summaries(db: &Db, provider: &dyn LlmProvider) {
    if let Err(e) = regenerate_daily_report_inner(db, provider, /* force = */ false) {
        warn!(error = %e, "auto L4 daily report generation failed");
    }
}

// ---- Public regenerate_* (manual UI / CLI buttons) ------------------

/// 强制重新生成今天的 L4 日报。
pub fn regenerate_daily_report(
    db: &Db,
    provider: &dyn LlmProvider,
) -> Result<Option<AggregateSummaryRow>> {
    regenerate_daily_report_inner(db, provider, /* force = */ true)
}

/// 强制重新生成当前 ISO 周的 L4 周报。
pub fn regenerate_weekly_report(
    db: &Db,
    provider: &dyn LlmProvider,
) -> Result<Option<AggregateSummaryRow>> {
    regenerate_weekly_report_inner(db, provider, /* force = */ true)
}

/// 强制重新生成当前自然月的 L4 月报。
pub fn regenerate_monthly_report(
    db: &Db,
    provider: &dyn LlmProvider,
) -> Result<Option<AggregateSummaryRow>> {
    regenerate_monthly_report_inner(db, provider, /* force = */ true)
}

/// 根据 scope_key 重新生成指定的日报/周报/月报。
/// scope_key 格式：`daily:2026-06-04` / `weekly:2026-W23` / `monthly:2026-06`。
pub fn regenerate_report_by_key(
    db: &Db,
    provider: &dyn LlmProvider,
    scope_key: &str,
) -> Result<Option<AggregateSummaryRow>> {
    let (scope_type, date_part) = scope_key
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid scope_key format: {}", scope_key))?;

    let spec = match scope_type {
        "daily" => daily_spec_for(date_part)?,
        "weekly" => weekly_spec_for(date_part)?,
        "monthly" => monthly_spec_for(date_part)?,
        other => return Err(anyhow!("unsupported scope_type: {}", other)),
    };
    // by_key 走低门槛：用户主动点了，至少有 1 条 L2 就出报告。
    let spec = ReportSpec {
        min_sessions: 1,
        ..spec
    };
    info!("regenerate_report_by_key: scope_key={}", scope_key);
    run_report(db, provider, &spec)
}

// ---- Inner impls ----------------------------------------------------

fn regenerate_daily_report_inner(
    db: &Db,
    provider: &dyn LlmProvider,
    force: bool,
) -> Result<Option<AggregateSummaryRow>> {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let spec = ReportSpec {
        min_sessions: if force { 1 } else { 2 },
        ..daily_spec_for(&today)?
    };
    run_report_unless_exists(db, provider, &spec, force)
}

fn regenerate_weekly_report_inner(
    db: &Db,
    provider: &dyn LlmProvider,
    force: bool,
) -> Result<Option<AggregateSummaryRow>> {
    let iso = Utc::now().iso_week();
    let date_part = format!("{}-W{:02}", iso.year(), iso.week());
    let spec = ReportSpec {
        min_sessions: if force { 1 } else { 3 },
        ..weekly_spec_for(&date_part)?
    };
    run_report_unless_exists(db, provider, &spec, force)
}

fn regenerate_monthly_report_inner(
    db: &Db,
    provider: &dyn LlmProvider,
    force: bool,
) -> Result<Option<AggregateSummaryRow>> {
    let today = Utc::now().date_naive();
    let (year, month) = (today.year(), today.month());
    let date_part = format!("{:04}-{:02}", year, month);
    let spec = ReportSpec {
        min_sessions: if force { 1 } else { 5 },
        ..monthly_spec_for(&date_part)?
    };
    run_report_unless_exists(db, provider, &spec, force)
}

// ---- Shared helpers -------------------------------------------------

/// Describes one L4 aggregation pass. Independent of scope type so
/// [`run_report`] can stay scope-agnostic.
struct ReportSpec {
    scope_type: &'static str,
    scope_key: String,
    after: String,
    before: String,
    period_label: String,
    fixed_title: String,
    min_sessions: usize,
}

fn daily_spec_for(date_part: &str) -> Result<ReportSpec> {
    Ok(ReportSpec {
        scope_type: "daily",
        scope_key: format!("daily:{}", date_part),
        after: format!("{}T00:00:00+00:00", date_part),
        before: format!("{}T23:59:59+00:00", date_part),
        period_label: format!("Daily {}", date_part),
        fixed_title: format!("日报 {}", date_part),
        min_sessions: 1,
    })
}

fn weekly_spec_for(date_part: &str) -> Result<ReportSpec> {
    let (year, week) = parse_iso_week(date_part)?;
    let start = NaiveDate::from_isoywd_opt(year, week, Weekday::Mon)
        .ok_or_else(|| anyhow!("invalid week: {}", date_part))?;
    let end = NaiveDate::from_isoywd_opt(year, week, Weekday::Sun)
        .ok_or_else(|| anyhow!("invalid week: {}", date_part))?;
    Ok(ReportSpec {
        scope_type: "weekly",
        scope_key: format!("weekly:{}", date_part),
        after: format!("{}T00:00:00+00:00", start),
        before: format!("{}T23:59:59+00:00", end),
        period_label: format!("Week {}", date_part),
        fixed_title: format!("周报 {}", date_part),
        min_sessions: 1,
    })
}

fn monthly_spec_for(date_part: &str) -> Result<ReportSpec> {
    let (year, month) = parse_year_month(date_part)?;
    let (after, before) =
        month_range(year, month).ok_or_else(|| anyhow!("invalid month: {}", date_part))?;
    Ok(ReportSpec {
        scope_type: "monthly",
        scope_key: format!("monthly:{}", date_part),
        after,
        before,
        period_label: format!("Monthly {}", date_part),
        fixed_title: format!("月报 {}", date_part),
        min_sessions: 1,
    })
}

/// Auto-trigger flavour: skip if an aggregate already exists for
/// this scope unless `force = true`.
fn run_report_unless_exists(
    db: &Db,
    provider: &dyn LlmProvider,
    spec: &ReportSpec,
    force: bool,
) -> Result<Option<AggregateSummaryRow>> {
    if !force
        && db
            .get_aggregate_summary(spec.scope_type, &spec.scope_key)
            .ok()
            .flatten()
            .is_some()
    {
        return Ok(None);
    }
    run_report(db, provider, spec)
}

/// Common aggregation flow: fetch sessions in range → collect L2
/// summaries → call summarize_period → upsert.
fn run_report(
    db: &Db,
    provider: &dyn LlmProvider,
    spec: &ReportSpec,
) -> Result<Option<AggregateSummaryRow>> {
    let sessions = match db.list_sessions_in_range(&spec.after, &spec.before) {
        Ok(s) if s.len() >= spec.min_sessions => s,
        Ok(s) => {
            warn!(
                "{}: only {} sessions in [{}, {}), below min={} — skipping",
                spec.scope_key,
                s.len(),
                spec.after,
                spec.before,
                spec.min_sessions
            );
            return Ok(None);
        }
        Err(e) => return Err(e),
    };

    let mut l2_summaries = Vec::with_capacity(sessions.len());
    for s in &sessions {
        if let Ok(Some(row)) = db.get_summary(&s.id, "L2_session") {
            l2_summaries.push(summarize::SessionSummary {
                title: row.title.unwrap_or_default(),
                summary: row.summary,
                topics: row.topics,
                decisions: row.decisions,
                project_name: None,
                corrected_project_path: None,
                intent: None,
            });
        }
    }
    if l2_summaries.is_empty() {
        warn!("{}: no L2 summaries available, skipping", spec.scope_key);
        return Ok(None);
    }

    let summary = summarize::summarize_period(provider, &spec.period_label, &l2_summaries)
        .map_err(|e| {
            warn!("L4 summarize failed ({}): {}", spec.scope_key, e);
            anyhow!(e)
        })?;
    db.upsert_aggregate_summary(AggregateSummaryUpsert {
        scope_type: spec.scope_type,
        scope_key: &spec.scope_key,
        title: Some(&spec.fixed_title),
        summary: &summary.summary,
        topics: &summary.topics,
        decisions: &summary.decisions,
        session_count: sessions.len() as i64,
    })?;
    db.get_aggregate_summary(spec.scope_type, &spec.scope_key)
}
