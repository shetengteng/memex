//! Top-level ingest orchestrator.
//!
//! [`run_ingest`] enumerates every enabled collector adapter, walks
//! its sessions via [`adapter::ingest_adapter`], then opportunistically
//! kicks off the LLM summary pyramid (L1 → L2 → L3 → L4). L5
//! ("主题线索") clustering stays manual; see [`threads`].

mod adapter;
mod period;
mod project_summaries;
mod reports;
mod summarize_levels;
mod threads;

use std::path::Path;

use anyhow::Result;

use crate::collector;
use crate::config::MemexConfig;
use crate::llm::provider::LlmProvider;
use crate::processor;
use crate::storage::db::Db;

pub use reports::{
    regenerate_daily_report, regenerate_monthly_report, regenerate_report_by_key,
    regenerate_weekly_report,
};
pub use summarize_levels::summarize_session_by_id;
pub use threads::{regenerate_threads, search_thread_by_query};

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
        match adapter::ingest_adapter(adapter.as_ref(), db, memex_dir) {
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

/// Run every LLM summary level we're willing to trigger from an
/// automatic ingest pass. L1/L2 are throttled (per-call sleep);
/// L3/L4 each run at most once per scope per pass so they don't
/// flood the local LLM.
fn try_summarize_new_sessions(db: &Db, memex_dir: &Path) {
    let config = match MemexConfig::load(memex_dir) {
        Ok(c) => c,
        Err(_) => return,
    };

    let provider = match crate::llm::select_provider_unified(db, &config.llm, memex_dir) {
        Some(p) => p,
        None => return,
    };
    let provider: &dyn LlmProvider = provider.as_ref();

    let throttle_ms = config.llm.summarize_interval_ms;

    summarize_levels::try_l1_chunk_summaries(db, provider, throttle_ms);
    summarize_levels::try_l2_session_summaries(
        db,
        provider,
        config.llm.summary_cooldown_secs,
        throttle_ms,
    );

    // L3 / L4 都是「整库一次性聚合」，每个 scope 只跑一次 LLM，
    // 不构成"短时间高频"压力，不需要节流。
    project_summaries::try_l3_project_summaries(db, provider);
    reports::try_l4_periodic_summaries(db, provider);
    reports::try_l4_weekly_summaries(db, provider);
    reports::try_l4_monthly_summaries(db, provider);
}
