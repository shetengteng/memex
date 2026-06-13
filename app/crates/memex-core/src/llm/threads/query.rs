//! Keyword-driven single-thread query via LLM. Used by the "Threads"
//! search box to ask the LLM "which of the last N sessions actually
//! discuss <X>?". Falls back to literal substring matching when the
//! LLM call/parse fails — see [`super::fallback::fallback_query_match`].

use anyhow::Result;
use serde::Deserialize;

use super::prompts::query_thread_system;
use super::{MAX_OUTPUT_TOKENS, MAX_SESSIONS_PER_BATCH, MAX_SUMMARY_CHARS, strip_code_fences};
use crate::llm::provider::{LlmProvider, LlmRequest};
use crate::llm::summarize::SessionSummary;
use crate::locale::PromptLocale;
use crate::storage::db::ThreadDraft;

#[derive(Debug, Clone, Deserialize)]
struct LlmQueryThread {
    #[serde(default)]
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    session_indices: Vec<usize>,
}

/// 按用户关键词，让 LLM 从一批 session 摘要里挑出相关 session。
/// 失败时调用方应使用 fallback：在标题/主题/摘要字面包含关键词。
pub fn query_threads_by_keyword(
    provider: &dyn LlmProvider,
    sessions: &[(String, SessionSummary)],
    query: &str,
) -> Result<ThreadDraft> {
    if sessions.is_empty() {
        return Ok(ThreadDraft {
            name: query.to_string(),
            summary: String::new(),
            session_ids: vec![],
        });
    }
    let batch: &[(String, SessionSummary)] = if sessions.len() > MAX_SESSIONS_PER_BATCH {
        &sessions[..MAX_SESSIONS_PER_BATCH]
    } else {
        sessions
    };

    let loc = PromptLocale::current();
    let prompt = build_query_prompt(batch, query, loc);

    let request = LlmRequest::with_prompt(prompt)
        .with_system(query_thread_system(loc))
        .with_max_tokens(MAX_OUTPUT_TOKENS);
    let response = provider.generate(&request)?;
    let cleaned = strip_code_fences(&response.text);
    let parsed: LlmQueryThread = serde_json::from_str(&cleaned).map_err(|e| {
        anyhow::anyhow!(
            "parse query thread JSON failed: {e}; raw: {}",
            &cleaned[..cleaned.len().min(200)]
        )
    })?;

    let mut seen = std::collections::HashSet::new();
    let mut session_ids = Vec::new();
    for idx in parsed.session_indices {
        if idx == 0 || idx > batch.len() {
            continue;
        }
        let sid = batch[idx - 1].0.clone();
        if seen.insert(sid.clone()) {
            session_ids.push(sid);
        }
    }

    let name = if parsed.name.trim().is_empty() {
        query.to_string()
    } else {
        parsed.name.trim().to_string()
    };

    Ok(ThreadDraft {
        name,
        summary: parsed.summary.trim().to_string(),
        session_ids,
    })
}

fn build_query_prompt(batch: &[(String, SessionSummary)], query: &str, loc: PromptLocale) -> String {
    let mut prompt = String::with_capacity(8000);
    let (kw_label, intro, untitled, topics_prefix, topics_suffix, topics_join, unknown_proj, footer) = match loc {
        PromptLocale::Zh => (
            "用户关键词：",
            "以下是若干条编程会话的摘要，每条带 1-based 编号和所属项目：\n\n",
            "(无标题)",
            "（主题：",
            "）",
            "、",
            "(未知项目)",
            "请按要求挑选与关键词相关的会话，输出 JSON。",
        ),
        PromptLocale::En => (
            "User keyword: ",
            "Below are programming session summaries, each with a 1-based index and project:\n\n",
            "(untitled)",
            " (topics: ",
            ")",
            ", ",
            "(unknown project)",
            "Pick the sessions relevant to the keyword and output JSON as required.",
        ),
    };
    prompt.push_str(&format!("{}{}\n\n", kw_label, query));
    prompt.push_str(intro);
    for (i, (_, summary)) in batch.iter().enumerate() {
        let n = i + 1;
        let title = if summary.title.is_empty() {
            untitled
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
            format!("{}{}{}", topics_prefix, summary.topics.join(topics_join), topics_suffix)
        };
        let project = summary
            .project_name
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .unwrap_or(unknown_proj);
        prompt.push_str(&format!(
            "[{}] project={} | {}{}\n  {}\n\n",
            n, project, title, topics, body,
        ));
    }
    prompt.push_str(footer);
    prompt
}
