use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::provider::{LlmProvider, LlmRequest};

const SUMMARY_SYSTEM: &str = "\
You are a technical session summarizer. Given a conversation between a user and an AI assistant, \
produce a concise summary. Output JSON with exactly these fields:
- title: one-line title (max 60 chars)
- summary: 2-4 sentence overview of what was accomplished
- topics: array of 1-5 topic keywords
- decisions: array of key decisions made (0-3 items)
Respond ONLY with valid JSON, no markdown fences.";

const CHUNK_SUMMARY_SYSTEM: &str = "\
You are a technical content summarizer. Given a piece of text from a coding session, \
produce a single concise sentence (max 120 chars) that captures the key information. \
Output ONLY the sentence, no quotes, no markdown, no extra formatting.";

const PROJECT_SUMMARY_SYSTEM: &str = "\
You are a project progress summarizer. Given session summaries from the same project, \
produce a project-level overview. Output JSON with exactly these fields:
- title: project name / one-line title (max 60 chars)
- summary: 3-5 sentence overview of project progress and current state
- topics: array of 1-8 topic keywords across all sessions
- decisions: array of key architectural/technical decisions (0-5 items)
Respond ONLY with valid JSON, no markdown fences.";

const PERIODIC_SUMMARY_SYSTEM: &str = "\
You are a work journal summarizer. Given session summaries from a time period, \
produce a periodic report. Output JSON with exactly these fields:
- title: period label (e.g. \"Daily Report 2026-06-01\")
- summary: 3-6 sentence overview of work accomplished in this period
- topics: array of 1-8 topic keywords
- decisions: array of key decisions made (0-5 items)
Respond ONLY with valid JSON, no markdown fences.";

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
    let prompt = format!("Summarize this content in one sentence:\n\n{}", truncated);
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
    prompt.push_str("Project session summaries:\n\n");
    for (i, s) in session_summaries.iter().enumerate() {
        let entry = format!(
            "Session {}: {}\n  {}\n  Topics: {}\n\n",
            i + 1,
            s.title,
            s.summary,
            s.topics.join(", ")
        );
        if prompt.len() + entry.len() > MAX_INPUT_CHARS {
            break;
        }
        prompt.push_str(&entry);
    }
    prompt.push_str("Produce a project-level summary as JSON.");
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
    prompt.push_str(&format!("Work sessions for {}:\n\n", period_label));
    for (i, s) in session_summaries.iter().enumerate() {
        let entry = format!(
            "Session {}: {}\n  {}\n  Decisions: {}\n\n",
            i + 1,
            s.title,
            s.summary,
            s.decisions.join("; ")
        );
        if prompt.len() + entry.len() > MAX_INPUT_CHARS {
            break;
        }
        prompt.push_str(&entry);
    }
    prompt.push_str("Produce a periodic work report as JSON.");
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
    prompt.push_str("Conversation:\n\n");

    let mut total_len = prompt.len();
    for (role, content) in messages {
        let header = format!("[{}]: ", role);
        let truncated = if content.len() > 1000 {
            format!("{}... (truncated)", &content[..1000])
        } else {
            content.clone()
        };
        let entry = format!("{}{}\n\n", header, truncated);

        if total_len + entry.len() > MAX_INPUT_CHARS {
            prompt.push_str("... (earlier messages omitted for brevity)\n");
            break;
        }
        prompt.push_str(&entry);
        total_len += entry.len();
    }

    prompt.push_str("\nSummarize this conversation as JSON.");
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
