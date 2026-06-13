//! 多层 LLM 摘要的入口模块。
//!
//! 拆分原则：
//! - [`prompts`]：4 个 system prompt（L1/L2/L3/L4）—— 调试 prompt 只改这里。
//! - [`parse`]：LLM 响应解析 + user-message prompt 拼装。
//! - [`period`]：L4 周期摘要（daily / weekly / monthly）的预算分档与浓缩逻辑。
//! - `mod.rs` 自身只保留：DTO（[`SessionSummary`]）、L1/L2/L3 入口函数、
//!   以及 L1 batching 之类的小常量。

mod parse;
mod period;
mod prompts;
#[cfg(test)]
mod tests;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::llm::provider::{LlmProvider, LlmRequest};
use crate::locale::PromptLocale;
use parse::{build_prompt, extract_first_sentence, parse_summary};
use prompts::{chunk_summary_system, project_summary_system, summary_system};

pub use period::summarize_period;

const MAX_INPUT_CHARS: usize = 8000;
const MAX_PERIOD_INPUT_CHARS: usize = 16000;
const MIN_CHUNK_CHARS_FOR_SUMMARY: usize = 200;
const L1_BATCH_SIZE: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub title: String,
    pub summary: String,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    /// L2 摘要新增字段：在 collector 给出 [`summarize_session`] 的
    /// `current_project_path` 之后，LLM 判断该路径漂移到了子目录
    /// （如 `tt-demo/src` / `repo/src/views/chat`）时，输出修正后的
    /// 完整项目根路径；路径已经合理时为 `None`。
    ///
    /// 与 [`SessionSummary::project_name`] 的区别：`project_name` 是 LLM
    /// 推断的**短名**（仅用于在 collector 没有路径时兜底）；本字段是
    /// **完整路径**，会强制覆盖 collector 写入的可能漂移的路径。
    #[serde(default)]
    pub corrected_project_path: Option<String>,
    /// L2 摘要新增字段：用户在本次会话中真正想达成的目标，一句话。
    /// L3 / L4 项目级与周期级摘要不强制要求 LLM 输出此字段，因此默认 None。
    #[serde(default)]
    pub intent: Option<String>,
}

pub fn summarize_session(
    provider: &dyn LlmProvider,
    messages: &[(String, String)],
    current_project_path: Option<&str>,
) -> Result<SessionSummary> {
    let loc = PromptLocale::current();
    let prompt = build_prompt(messages, current_project_path, loc);
    let request = LlmRequest::with_prompt(prompt).with_system(summary_system(loc));
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

pub fn summarize_chunk(provider: &dyn LlmProvider, content: &str) -> Result<String> {
    if content.len() < MIN_CHUNK_CHARS_FOR_SUMMARY {
        return Ok(extract_first_sentence(content, 120));
    }
    let loc = PromptLocale::current();
    let truncated_marker = match loc {
        PromptLocale::Zh => "... （已截断）",
        PromptLocale::En => "... (truncated)",
    };
    let truncated = if content.len() > 2000 {
        format!(
            "{}{}",
            &content[..content.floor_char_boundary(2000)],
            truncated_marker
        )
    } else {
        content.to_string()
    };
    let prompt = match loc {
        PromptLocale::Zh => format!("请用一句话总结以下内容：\n\n{}", truncated),
        PromptLocale::En => format!("Summarize the following in one sentence:\n\n{}", truncated),
    };
    let request = LlmRequest::with_prompt(prompt).with_system(chunk_summary_system(loc));
    match provider.generate(&request) {
        Ok(response) => {
            let s = response.text.trim().trim_matches('"').to_string();
            Ok(if s.len() > 120 {
                s.chars().take(120).collect()
            } else {
                s
            })
        }
        Err(_) => Ok(extract_first_sentence(content, 120)),
    }
}

pub fn summarize_project(
    provider: &dyn LlmProvider,
    session_summaries: &[SessionSummary],
) -> Result<SessionSummary> {
    let loc = PromptLocale::current();
    let (intro, session_label, summary_label, topics_label, topics_join, footer) = match loc {
        PromptLocale::Zh => (
            "以下是同一个项目的多个会话摘要：\n\n",
            "会话",
            "摘要",
            "主题",
            "、",
            "请输出一个项目级总览的 JSON。",
        ),
        PromptLocale::En => (
            "Below are multiple session summaries from the same project:\n\n",
            "Session",
            "Summary",
            "Topics",
            ", ",
            "Output the project-level rollup as JSON.",
        ),
    };
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);
    prompt.push_str(intro);
    for (i, s) in session_summaries.iter().enumerate() {
        let entry = format!(
            "{} {}: {}\n  {}: {}\n  {}: {}\n\n",
            session_label,
            i + 1,
            s.title,
            summary_label,
            s.summary,
            topics_label,
            s.topics.join(topics_join),
        );
        if prompt.len() + entry.len() > MAX_INPUT_CHARS {
            break;
        }
        prompt.push_str(&entry);
    }
    prompt.push_str(footer);
    let request = LlmRequest::with_prompt(prompt).with_system(project_summary_system(loc));
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

pub const fn min_chunk_chars() -> usize {
    MIN_CHUNK_CHARS_FOR_SUMMARY
}

pub const fn l1_batch_size() -> usize {
    L1_BATCH_SIZE
}
