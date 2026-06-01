use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::provider::{LlmProvider, LlmRequest};

const SUMMARY_SYSTEM: &str = "\
你是一个面向技术开发场景的会话摘要助手。输入是用户与 AI 助手的一段对话，\
请生成一段简洁、有结构的摘要。输出 JSON，字段如下：
- title: 一行标题，不超过 60 个字符
- summary: 用 2-4 句话概括完成了什么、做了哪些关键决策
- topics: 1-5 个主题关键词组成的数组
- decisions: 0-3 个关键决策组成的数组
所有自然语言字段（title、summary、topics、decisions）都必须使用简体中文，\
无论输入是什么语言。保持技术标识原样：文件路径、命令名、函数名、英文缩写（SQL/HTTP/API 等）\
不要翻译。\
只输出合法 JSON，不要带 markdown 代码块标记。";

const CHUNK_SUMMARY_SYSTEM: &str = "\
你是一个面向技术开发场景的文本摘要助手。输入是编程会话中的一段文本，\
请用一句话（简体中文，不超过 120 字符）抓住核心信息。\
保持技术标识原样：文件路径、命令、代码符号不要翻译。\
只输出这一句话，不要带引号、markdown 或任何额外格式。";

const PROJECT_SUMMARY_SYSTEM: &str = "\
你是一个项目进展摘要助手。输入是同一个项目内多个会话的摘要，\
请生成项目级别的总览。输出 JSON，字段如下：
- title: 项目名或一行标题，不超过 60 个字符
- summary: 用 3-5 句话概括项目当前的进展、关键状态
- topics: 1-8 个覆盖所有会话的主题关键词数组
- decisions: 0-5 个关键架构/技术决策数组
所有自然语言字段都必须使用简体中文，无论输入语言是什么。\
保持技术标识原样：文件路径、命令名、函数名、英文缩写（SQL/HTTP/API 等）不要翻译。\
只输出合法 JSON，不要带 markdown 代码块标记。";

const PERIODIC_SUMMARY_SYSTEM: &str = "\
你是一个工作日记摘要助手。输入是某个时间段内多个会话的摘要，\
请生成周期性工作报告。输出 JSON，字段如下：
- title: 周期标签，如 \"日报 2026-06-01\" 或 \"周报 2026-W22\"
- summary: 用 3-6 句话概括这段时间内完成的工作
- topics: 1-8 个主题关键词数组
- decisions: 0-5 个关键决策数组
所有自然语言字段都必须使用简体中文，无论输入会话摘要是什么语言（即使输入是英文，\
输出也必须是中文）。保持技术标识原样：文件路径、命令名、函数名、英文缩写（SQL/HTTP/API 等）\
不要翻译。\
只输出合法 JSON，不要带 markdown 代码块标记。";

const MAX_INPUT_CHARS: usize = 8000;
const MIN_CHUNK_CHARS_FOR_SUMMARY: usize = 200;
const L1_BATCH_SIZE: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub title: String,
    pub summary: String,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
}

pub fn summarize_session(
    provider: &dyn LlmProvider,
    messages: &[(String, String)],
) -> Result<SessionSummary> {
    let prompt = build_prompt(messages);
    let request = LlmRequest::with_prompt(prompt).with_system(SUMMARY_SYSTEM);
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

pub fn summarize_chunk(provider: &dyn LlmProvider, content: &str) -> Result<String> {
    if content.len() < MIN_CHUNK_CHARS_FOR_SUMMARY {
        return Ok(extract_first_sentence(content, 120));
    }
    let truncated = if content.len() > 2000 {
        format!("{}... (truncated)", &content[..content.floor_char_boundary(2000)])
    } else {
        content.to_string()
    };
    let prompt = format!("请用一句话总结以下内容：\n\n{}", truncated);
    let request = LlmRequest::with_prompt(prompt).with_system(CHUNK_SUMMARY_SYSTEM);
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
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);
    prompt.push_str("以下是同一个项目的多个会话摘要：\n\n");
    for (i, s) in session_summaries.iter().enumerate() {
        let entry = format!(
            "会话 {}：{}\n  摘要：{}\n  主题：{}\n\n",
            i + 1,
            s.title,
            s.summary,
            s.topics.join("、")
        );
        if prompt.len() + entry.len() > MAX_INPUT_CHARS {
            break;
        }
        prompt.push_str(&entry);
    }
    prompt.push_str("请输出一个项目级总览的 JSON。");
    let request = LlmRequest::with_prompt(prompt).with_system(PROJECT_SUMMARY_SYSTEM);
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

pub fn summarize_period(
    provider: &dyn LlmProvider,
    period_label: &str,
    session_summaries: &[SessionSummary],
) -> Result<SessionSummary> {
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);
    prompt.push_str(&format!("以下是 {} 期间的工作会话摘要：\n\n", period_label));
    for (i, s) in session_summaries.iter().enumerate() {
        let entry = format!(
            "会话 {}：{}\n  摘要：{}\n  关键决策：{}\n\n",
            i + 1,
            s.title,
            s.summary,
            s.decisions.join("；")
        );
        if prompt.len() + entry.len() > MAX_INPUT_CHARS {
            break;
        }
        prompt.push_str(&entry);
    }
    prompt.push_str("请输出一份周期性工作报告的 JSON。");
    let request = LlmRequest::with_prompt(prompt).with_system(PERIODIC_SUMMARY_SYSTEM);
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

pub const fn min_chunk_chars() -> usize {
    MIN_CHUNK_CHARS_FOR_SUMMARY
}

pub const fn l1_batch_size() -> usize {
    L1_BATCH_SIZE
}

fn build_prompt(messages: &[(String, String)]) -> String {
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);
    prompt.push_str("以下是一段对话：\n\n");

    let mut total_len = prompt.len();
    for (role, content) in messages {
        let header = format!("[{}]：", role);
        let truncated = if content.len() > 1000 {
            let end = content.char_indices()
                .take_while(|(i, _)| *i < 1000)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(content.len().min(1000));
            format!("{}…（已截断）", &content[..end])
        } else {
            content.clone()
        };
        let entry = format!("{}{}\n\n", header, truncated);

        if total_len + entry.len() > MAX_INPUT_CHARS {
            prompt.push_str("…（为节省篇幅省略了较早的消息）\n");
            break;
        }
        prompt.push_str(&entry);
        total_len += entry.len();
    }

    prompt.push_str("\n请把这段对话总结为 JSON。");
    prompt
}

fn parse_summary(text: &str) -> Result<SessionSummary> {
    let cleaned = text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(summary) = serde_json::from_str::<SessionSummary>(cleaned) {
        return Ok(summary);
    }

    Ok(SessionSummary {
        title: extract_first_sentence(text, 60),
        summary: text.chars().take(500).collect(),
        topics: Vec::new(),
        decisions: Vec::new(),
    })
}

fn extract_first_sentence(text: &str, max_len: usize) -> String {
    let end = text.find('.').map(|i| i + 1).unwrap_or(text.len());
    let sentence: String = text.chars().take(end.min(max_len)).collect();
    sentence.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::{LlmProvider, LlmRequest, LlmResponse};

    struct MockProvider {
        response: String,
    }

    impl MockProvider {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        fn is_available(&self) -> bool {
            true
        }
        fn generate(&self, _request: &LlmRequest) -> Result<LlmResponse> {
            Ok(LlmResponse {
                text: self.response.clone(),
                model: "mock".into(),
                tokens_used: 10,
            })
        }
    }

    #[test]
    fn test_parse_summary_valid_json() {
        let json = r#"{"title":"Fix auth bug","summary":"Fixed JWT token parsing.","topics":["auth","jwt"],"decisions":["use RS256"]}"#;
        let s = parse_summary(json).unwrap();
        assert_eq!(s.title, "Fix auth bug");
        assert_eq!(s.topics.len(), 2);
    }

    #[test]
    fn test_parse_summary_with_fences() {
        let text =
            "```json\n{\"title\":\"Test\",\"summary\":\"S\",\"topics\":[],\"decisions\":[]}\n```";
        let s = parse_summary(text).unwrap();
        assert_eq!(s.title, "Test");
    }

    #[test]
    fn test_parse_summary_fallback() {
        let text = "This is not valid JSON but a plain text response.";
        let s = parse_summary(text).unwrap();
        assert!(!s.title.is_empty());
    }

    #[test]
    fn test_build_prompt_truncation() {
        let messages: Vec<(String, String)> = (0..100)
            .map(|i| ("user".to_string(), format!("Message {} with content", i)))
            .collect();
        let prompt = build_prompt(&messages);
        assert!(prompt.len() <= MAX_INPUT_CHARS + 100);
    }

    #[test]
    fn test_extract_first_sentence() {
        assert_eq!(
            extract_first_sentence("Hello world. More stuff.", 60),
            "Hello world."
        );
        assert_eq!(extract_first_sentence("Short", 60), "Short");
    }

    #[test]
    fn test_summarize_chunk_short_content_no_llm() {
        let provider = MockProvider::new("should not be called");
        let short = "Quick fix applied.";
        let result = summarize_chunk(&provider, short).unwrap();
        assert_eq!(result, "Quick fix applied.");
    }

    #[test]
    fn test_summarize_chunk_uses_llm_for_long_content() {
        let provider = MockProvider::new("Implemented Redis caching layer for session data.");
        let long_content = "a]".repeat(200);
        let result = summarize_chunk(&provider, &long_content).unwrap();
        assert_eq!(result, "Implemented Redis caching layer for session data.");
    }

    #[test]
    fn test_summarize_chunk_truncates_long_response() {
        let provider = MockProvider::new(&"x".repeat(200));
        let content = "a".repeat(300);
        let result = summarize_chunk(&provider, &content).unwrap();
        assert!(result.len() <= 120);
    }

    #[test]
    fn test_summarize_project() {
        let provider = MockProvider::new(
            r#"{"title":"Memex Core","summary":"Built core features.","topics":["rust","search"],"decisions":["use FTS5"]}"#,
        );
        let sessions = vec![
            SessionSummary {
                title: "Add search".into(),
                summary: "Implemented FTS5 search.".into(),
                topics: vec!["search".into()],
                decisions: vec![],
            },
            SessionSummary {
                title: "Add adapters".into(),
                summary: "Added Claude and Cursor adapters.".into(),
                topics: vec!["adapters".into()],
                decisions: vec![],
            },
        ];
        let result = summarize_project(&provider, &sessions).unwrap();
        assert_eq!(result.title, "Memex Core");
        assert!(!result.topics.is_empty());
    }

    #[test]
    fn test_summarize_period() {
        let provider = MockProvider::new(
            r#"{"title":"Daily Report 2026-06-01","summary":"Worked on features.","topics":["dev"],"decisions":[]}"#,
        );
        let sessions = vec![SessionSummary {
            title: "Feature work".into(),
            summary: "Built new feature.".into(),
            topics: vec!["feature".into()],
            decisions: vec!["use trait pattern".into()],
        }];
        let result = summarize_period(&provider, "2026-06-01", &sessions).unwrap();
        assert!(result.title.contains("Daily Report"));
    }
}
