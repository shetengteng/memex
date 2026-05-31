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

const MAX_INPUT_CHARS: usize = 8000;

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
}
