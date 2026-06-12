//! L5 thread clustering via LLM. Given a batch of session summaries,
//! ask the LLM to group them into 1..=12 high-level threads. Failure
//! cases here fall back to the deterministic bucket algorithm in
//! [`super::fallback`].

use anyhow::Result;
use serde::Deserialize;

use super::prompts::THREAD_CLUSTERING_SYSTEM;
use super::{
    MAX_OUTPUT_TOKENS, MAX_SESSIONS_PER_BATCH, MAX_SESSIONS_PER_THREAD, MAX_SUMMARY_CHARS,
    MAX_THREADS_PER_RESPONSE, strip_code_fences,
};
use crate::llm::provider::{LlmProvider, LlmRequest};
use crate::llm::summarize::SessionSummary;
use crate::storage::db::ThreadDraft;

/// LLM 解析得到的单个 thread 草稿（带 1-based 输入序号）。
#[derive(Debug, Clone, Deserialize)]
pub(super) struct LlmThread {
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    session_indices: Vec<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct LlmThreadResponse {
    #[serde(default)]
    pub(super) threads: Vec<LlmThread>,
}

/// 把一批 (session_id, SessionSummary) 喂给 LLM，让它聚类成 thread。
/// 返回值是落库前的 ThreadDraft 列表。失败时返回 Err，调用方应使用 fallback。
pub fn cluster_threads(
    provider: &dyn LlmProvider,
    sessions: &[(String, SessionSummary)],
) -> Result<Vec<ThreadDraft>> {
    if sessions.is_empty() {
        return Ok(Vec::new());
    }

    let batch: &[(String, SessionSummary)] = if sessions.len() > MAX_SESSIONS_PER_BATCH {
        &sessions[..MAX_SESSIONS_PER_BATCH]
    } else {
        sessions
    };

    let prompt = build_clustering_prompt(batch);
    let request = LlmRequest::with_prompt(prompt)
        .with_system(THREAD_CLUSTERING_SYSTEM)
        .with_max_tokens(MAX_OUTPUT_TOKENS);
    let response = provider.generate(&request)?;

    let parsed = parse_thread_response(&response.text)?;
    Ok(map_to_drafts(&parsed.threads, batch))
}

pub(super) fn build_clustering_prompt(batch: &[(String, SessionSummary)]) -> String {
    let mut prompt = String::with_capacity(8000);
    prompt.push_str("以下是若干条编程会话的摘要，每条带一个 1-based 编号和所属项目：\n\n");
    for (i, (_, summary)) in batch.iter().enumerate() {
        let n = i + 1;
        let title = if summary.title.is_empty() {
            "(无标题)"
        } else {
            &summary.title
        };
        let body = if summary.summary.chars().count() > MAX_SUMMARY_CHARS {
            let s: String = summary.summary.chars().take(MAX_SUMMARY_CHARS).collect();
            format!("{}…", s)
        } else {
            summary.summary.clone()
        };
        let topics = if summary.topics.is_empty() {
            String::new()
        } else {
            format!("（主题：{}）", summary.topics.join("、"))
        };
        let project = summary
            .project_name
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .unwrap_or("(未知项目)");
        prompt.push_str(&format!(
            "[{}] project={} | {}{}\n  {}\n\n",
            n, project, title, topics, body,
        ));
    }
    prompt.push_str(
        "请把上面的会话聚类成主题线索，按要求输出 JSON。\
         记住：不同 project 的 session 默认不能聚到同一个 thread。",
    );
    prompt
}

pub(super) fn parse_thread_response(text: &str) -> Result<LlmThreadResponse> {
    let cleaned = strip_code_fences(text);
    serde_json::from_str::<LlmThreadResponse>(&cleaned).map_err(|e| {
        anyhow::anyhow!(
            "parse thread JSON failed: {e}; raw: {}",
            &cleaned[..cleaned.len().min(200)]
        )
    })
}

pub(super) fn map_to_drafts(
    llm_threads: &[LlmThread],
    batch: &[(String, SessionSummary)],
) -> Vec<ThreadDraft> {
    let mut drafts = Vec::with_capacity(llm_threads.len());
    for t in llm_threads.iter().take(MAX_THREADS_PER_RESPONSE) {
        let name = t.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        let mut session_ids: Vec<String> = t
            .session_indices
            .iter()
            .filter_map(|&i| {
                if i == 0 || i > batch.len() {
                    None
                } else {
                    Some(batch[i - 1].0.clone())
                }
            })
            .collect();
        session_ids.sort();
        session_ids.dedup();
        if session_ids.is_empty() {
            continue;
        }
        if session_ids.len() > MAX_SESSIONS_PER_THREAD {
            session_ids.truncate(MAX_SESSIONS_PER_THREAD);
        }
        drafts.push(ThreadDraft {
            name,
            summary: t.summary.trim().to_string(),
            session_ids,
        });
    }
    drafts
}
