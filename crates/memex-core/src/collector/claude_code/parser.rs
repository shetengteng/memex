//! Parse a Claude Code rollout JSONL line into a `RawMessage`.
//! Mirrors tars-ai-butler `tars/adapters/claude_code.py` content extraction.

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::storage::models::{RawMessage, Role};

#[derive(Debug, Deserialize)]
pub(super) struct ClaudeMessage {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub msg_type: Option<String>,
    pub role: Option<String>,
    pub message: Option<ClaudeMessageBody>,
    pub timestamp: Option<String>,
    pub uuid: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ClaudeMessageBody {
    pub role: Option<String>,
    pub content: Option<serde_json::Value>,
}

pub(super) fn convert_claude_message(
    msg: &ClaudeMessage,
    session_id: &str,
    offset: u64,
) -> Option<RawMessage> {
    let role_str = msg
        .role
        .as_deref()
        .or_else(|| msg.message.as_ref().and_then(|m| m.role.as_deref()))?;

    let role = match role_str {
        "human" | "user" => Role::User,
        "assistant" => Role::Assistant,
        "system" => Role::System,
        "tool" => Role::Tool,
        _ => return None,
    };

    let content = extract_content(msg)?;
    if content.trim().is_empty() {
        return None;
    }

    let timestamp = msg
        .timestamp
        .as_deref()
        .and_then(|ts| DateTime::parse_from_rfc3339(ts).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let id = msg.uuid.clone().unwrap_or_else(|| {
        blake3::hash(
            format!(
                "{}{}{}",
                session_id,
                offset,
                super::super::safe_prefix(&content, 100)
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string()
    });

    Some(RawMessage {
        id,
        session_id: session_id.to_string(),
        role,
        content,
        timestamp,
        source_offset: offset,
    })
}

fn extract_content(msg: &ClaudeMessage) -> Option<String> {
    let body = msg.message.as_ref()?;
    let content_val = body.content.as_ref()?;
    match content_val {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}
