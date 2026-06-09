//! L1 (chunk) and L2 (session) summary generation triggered by
//! [`super::try_summarize_new_sessions`]. Throttling lives here so the
//! local Ollama / LM Studio process isn't flooded by a single ingest
//! pass.

use tracing::warn;

use crate::llm::provider::LlmProvider;
use crate::llm::summarize;
use crate::storage::db::Db;

/// Iterate over `chunks_without_summary` and produce a one-line L1
/// summary per chunk. Sleeps `throttle_ms` between calls (except
/// after the last one) to avoid hammering the local LLM.
pub(super) fn try_l1_chunk_summaries(db: &Db, provider: &dyn LlmProvider, throttle_ms: u64) {
    let min_tokens = (summarize::min_chunk_chars() / 4) as u32;
    let chunks = match db.chunks_without_summary(min_tokens, summarize::l1_batch_size()) {
        Ok(c) => c,
        Err(_) => return,
    };

    let total = chunks.len();
    for (i, (chunk_id, content, _)) in chunks.into_iter().enumerate() {
        match summarize::summarize_chunk(provider, &content) {
            Ok(s) => {
                if let Err(e) = db.update_chunk_summary(chunk_id, &s) {
                    warn!(chunk_id, error = %e, "failed to persist L1 chunk summary");
                }
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

/// L2 session summaries. Combines two staleness signals:
///   - A. `sessions.message_count > summaries.message_count_at_creation`
///     means new messages have arrived since the last summary, so we
///     regenerate.
///   - B. `sessions.updated_at` must be at least `cool_down_secs` old
///     to avoid summarizing the same session on every ingest pass
///     during interactive editing.
pub(super) fn try_l2_session_summaries(
    db: &Db,
    provider: &dyn LlmProvider,
    cool_down_secs: u64,
    throttle_ms: u64,
) {
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

/// Summarize a single session by id. Exposed as `pub` because the
/// menubar's "重新生成 L2" button and the CLI's
/// `memex summarize <session>` both call it directly.
pub fn summarize_session_by_id(db: &Db, provider: &dyn LlmProvider, session_id: &str) -> bool {
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
            if let Err(e) = db.upsert_summary(crate::storage::db::SummaryUpsert {
                session_id,
                level: "L2_session",
                title: Some(&summary.title),
                summary: &summary.summary,
                topics: &summary.topics,
                decisions: &summary.decisions,
                message_count_at_creation,
            }) {
                warn!(
                    session_id = &session_id[..8.min(session_id.len())],
                    error = %e,
                    "failed to persist L2 session summary"
                );
                return false;
            }
            if let Some(ref name) = summary.project_name
                && let Err(e) = db.update_session_project_path(session_id, name)
            {
                warn!(
                    session_id = &session_id[..8.min(session_id.len())],
                    error = %e,
                    "failed to persist session project_path"
                );
            }
            // 每次重生成都覆盖 sessions.intent，None 时显式写入 NULL，
            // 避免「重新生成后旧 intent 文本继续留在 UI 上」的脏数据。
            if let Err(e) = db.update_session_intent(session_id, summary.intent.as_deref()) {
                warn!(
                    session_id = &session_id[..8.min(session_id.len())],
                    error = %e,
                    "failed to persist session intent"
                );
            }
            true
        }
        Err(e) => {
            warn!(
                "L2 summarize failed for session {}: {}",
                &session_id[..8.min(session_id.len())],
                e
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Instant;

    use crate::llm::provider::{LlmRequest, LlmResponse};
    use crate::storage::models::{Chunk, ChunkMetadata, ChunkType};

    /// 节流（throttle）回归：
    /// 自动模式的 `try_l1_chunk_summaries` 应该在每两次 LLM 调用之间
    /// sleep `llm.summarize_interval_ms`。
    /// 我们用一个会记录调用时刻的 mock provider 验证间隔确实 ≥ throttle。
    ///
    /// 为了让测试跑得快，throttle 设 50ms，调用 3 次 → 至少 100ms 间隔。
    /// 真实运行时配置为 2000ms，行为相同。
    #[test]
    fn throttle_inserts_sleep_between_l1_chunk_summaries() {
        struct TickProvider {
            ticks: Mutex<Vec<Instant>>,
        }
        impl LlmProvider for TickProvider {
            fn name(&self) -> &str {
                "tick"
            }
            fn is_available(&self) -> bool {
                true
            }
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
        db.insert_session("s1", "claude_code", Some("/p"), "/f.jsonl", 0, 0)
            .unwrap();
        for i in 0..3 {
            db.insert_message(
                &format!("m{i}"),
                "s1",
                "user",
                &format!("message {i} with enough content to summarize lalalalala"),
                None,
                i,
                blake3::hash(format!("msg{i}").as_bytes()).to_hex().as_str(),
            )
            .unwrap();
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
            })
            .unwrap();
        }

        let provider = TickProvider {
            ticks: Mutex::new(Vec::new()),
        };

        let start = Instant::now();
        try_l1_chunk_summaries(&db, &provider, /* throttle_ms = */ 50);
        let elapsed = start.elapsed();

        let ticks = provider.ticks.lock().unwrap();
        assert_eq!(ticks.len(), 3, "应该跑了 3 次 LLM 调用");
        for i in 1..ticks.len() {
            let gap = ticks[i].duration_since(ticks[i - 1]);
            assert!(
                gap.as_millis() >= 45,
                "第 {} 次和第 {} 次 LLM 调用间隔应该 ≥ 50ms，实际 {:?}",
                i - 1,
                i,
                gap
            );
        }
        assert!(
            elapsed.as_millis() >= 90,
            "3 次调用应该至少 100ms，实际 {:?}",
            elapsed
        );
    }

    /// 节流 = 0 时退化为旧行为（不 sleep），确保我们不破坏现有用户配置。
    #[test]
    fn throttle_zero_disables_sleep() {
        struct FastProvider {
            count: Mutex<usize>,
        }
        impl LlmProvider for FastProvider {
            fn name(&self) -> &str {
                "fast"
            }
            fn is_available(&self) -> bool {
                true
            }
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
        db.insert_session("s1", "claude_code", Some("/p"), "/f.jsonl", 0, 0)
            .unwrap();
        for i in 0..5 {
            db.insert_message(
                &format!("m{i}"),
                "s1",
                "user",
                &format!("hello {i}"),
                None,
                i,
                blake3::hash(format!("h{i}").as_bytes()).to_hex().as_str(),
            )
            .unwrap();
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
            })
            .unwrap();
        }

        let provider = FastProvider {
            count: Mutex::new(0),
        };

        let start = Instant::now();
        try_l1_chunk_summaries(&db, &provider, /* throttle_ms = */ 0);
        let elapsed = start.elapsed();

        assert_eq!(*provider.count.lock().unwrap(), 5, "应跑 5 个 chunk");
        assert!(
            elapsed.as_millis() < 200,
            "throttle=0 时 5 次调用不应该花太久，实际 {:?}",
            elapsed
        );
    }
}
