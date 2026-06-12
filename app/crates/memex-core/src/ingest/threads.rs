//! L5 thread clustering: ask the LLM to group recent L2 session
//! summaries into named "线索" (threads) and persist the resulting
//! many-to-many mapping.
//!
//! Both entry points are manual: `regenerate_threads` is called by
//! the Library "重新聚类" button (and the CLI), and
//! `search_thread_by_query` is the natural-language query path.
//! Neither is wired into [`super::try_summarize_new_sessions`] because
//! a single clustering pass already costs ~16k tokens of input and
//! running it every ingest tick would punish local Ollama users.

use anyhow::Result;
use tracing::warn;

use crate::llm::provider::LlmProvider;
use crate::llm::summarize;
use crate::storage::db::Db;

/// Pull the latest 80 sessions with L2 summaries, hand them to the
/// LLM clustering prompt, and upsert the resulting threads. Returns
/// the count of threads actually persisted.
///
/// Falls back to topic-bucket clustering when:
///   - the LLM is unavailable / errors, or
///   - the LLM returned zero threads (so the UI always has something
///     to show after a successful click).
pub fn regenerate_threads(db: &Db, provider: &dyn LlmProvider) -> Result<usize> {
    let batch = collect_recent_l2(db, 80)?;
    if batch.is_empty() {
        return Ok(0);
    }

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

    let mut ok = 0usize;
    for d in &drafts {
        match db.upsert_thread_with_sessions(d) {
            Ok(_) => ok += 1,
            Err(e) => warn!("upsert thread '{}' failed: {}", d.name, e),
        }
    }
    Ok(ok)
}

/// Take a user-supplied keyword, ask the LLM to pick sessions whose
/// L2 summary relates to it, and upsert the result as a single thread.
/// Returns the new thread's id.
///
/// Falls back to literal substring matching when the LLM fails.
pub fn search_thread_by_query(db: &Db, provider: &dyn LlmProvider, query: &str) -> Result<i64> {
    let q = query.trim();
    if q.is_empty() {
        anyhow::bail!("query is empty");
    }

    let batch = collect_recent_l2(db, 80)?;
    if batch.is_empty() {
        anyhow::bail!("最近 80 个会话都没有 L2 摘要——先生成 L2 摘要");
    }

    let draft = match crate::llm::threads::query_threads_by_keyword(provider, &batch, q) {
        Ok(d) => d,
        Err(e) => {
            warn!("L5 LLM 关键词检索失败，使用字面匹配 fallback: {}", e);
            crate::llm::threads::fallback_query_match(&batch, q)
        }
    };

    if draft.session_ids.is_empty() {
        anyhow::bail!("未找到与「{}」相关的会话", q);
    }
    let thread_id = db.upsert_thread_with_sessions(&draft)?;
    Ok(thread_id)
}

/// Gather the most recent `limit` sessions that have an L2 summary,
/// shaped as the `(session_id, SessionSummary)` pairs both clustering
/// and keyword-query prompts consume.
///
/// `project_name` is filled from the last non-empty path segment so
/// the clustering prompt can avoid mixing chats from different
/// projects that happen to discuss similar files (e.g. two repos with
/// a `prompts.txt`).
fn collect_recent_l2(db: &Db, limit: usize) -> Result<Vec<(String, summarize::SessionSummary)>> {
    let sessions = db.list_sessions_paged(limit, 0)?;
    if sessions.is_empty() {
        return Ok(Vec::new());
    }

    let mut batch = Vec::with_capacity(sessions.len());
    for s in &sessions {
        let row = match db.get_summary(&s.id, "L2_session") {
            Ok(Some(r)) => r,
            _ => continue,
        };
        let project_name = s.project_path.as_deref().and_then(|p| {
            p.rsplit('/')
                .find(|seg| !seg.is_empty())
                .map(|s| s.to_string())
        });
        batch.push((
            s.id.clone(),
            summarize::SessionSummary {
                title: row.title.unwrap_or_default(),
                summary: row.summary,
                topics: row.topics,
                decisions: row.decisions,
                project_name,
                corrected_project_path: None,
                intent: None,
            },
        ));
    }
    Ok(batch)
}
