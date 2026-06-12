//! L3 project-scope aggregation. Bundles every L2 session summary
//! that shares a `project_path` and asks the LLM for a project-level
//! summary (one per project, written to `aggregate_summaries`).

use tracing::warn;

use crate::llm::provider::LlmProvider;
use crate::llm::summarize;
use crate::storage::db::Db;

/// For each distinct `project_path`, if we don't already have an
/// L3 aggregate summary and at least 2 sessions have L2 summaries,
/// ask the LLM to combine them and persist the result.
///
/// Existing aggregates are not refreshed here; the user has to hit
/// the "重新生成" button (which calls into queries directly).
pub(super) fn try_l3_project_summaries(db: &Db, provider: &dyn LlmProvider) {
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
                    corrected_project_path: None,
                    intent: None,
                });
            }
        }
        if l2_summaries.len() < 2 {
            continue;
        }

        match summarize::summarize_project(provider, &l2_summaries) {
            Ok(summary) => {
                if let Err(e) =
                    db.upsert_aggregate_summary(crate::storage::db::AggregateSummaryUpsert {
                        scope_type: "project",
                        scope_key: &project,
                        title: Some(&summary.title),
                        summary: &summary.summary,
                        topics: &summary.topics,
                        decisions: &summary.decisions,
                        session_count: sessions.len() as i64,
                    })
                {
                    warn!(project = %project, error = %e, "failed to persist L3 project summary");
                }
            }
            Err(e) => {
                warn!("L3 project summarize failed for {}: {}", project, e);
            }
        }
    }
}
