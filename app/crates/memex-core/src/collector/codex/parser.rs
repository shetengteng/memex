//! 按行解析器：根据 session JSONL 每行的顶层 `type` 字段分发，
//! 把 `response_item` / `event_msg` 这两类 payload 转换成 `RawMessage`。
//! 对齐 tars-ai-butler `tars/adapters/codex.py` 的内容提取逻辑。

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::storage::models::{RawMessage, Role};

#[derive(Debug, Deserialize)]
pub(super) struct SessionEntry {
    #[serde(rename = "type")]
    pub entry_type: Option<String>,
    pub timestamp: Option<String>,
    pub payload: Option<serde_json::Value>,
}

pub(super) fn build_message_from_response(
    payload: &serde_json::Value,
    session_id: &str,
    offset: u64,
    timestamp: Option<DateTime<Utc>>,
) -> Option<RawMessage> {
    let role_str = payload.get("role").and_then(|v| v.as_str())?;
    let role = match role_str {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "system" => Role::System,
        "tool" => Role::Tool,
        _ => return None,
    };

    let content_array = payload.get("content").and_then(|v| v.as_array())?;
    let text = match role {
        Role::User => extract_user_text(content_array),
        _ => extract_assistant_text(content_array),
    };

    let text = text.trim().to_string();
    if text.is_empty() {
        return None;
    }

    Some(RawMessage {
        id: hash_id("codex", session_id, offset, &text),
        session_id: session_id.to_string(),
        role,
        content: text,
        timestamp,
        source_offset: offset,
    })
}

pub(super) fn build_message_from_event(
    payload: &serde_json::Value,
    session_id: &str,
    offset: u64,
    timestamp: Option<DateTime<Utc>>,
) -> Option<RawMessage> {
    if payload.get("type").and_then(|v| v.as_str()) != Some("last_agent_message") {
        return None;
    }
    let text = payload
        .get("last_agent_message")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())?;

    Some(RawMessage {
        id: hash_id("codex-event", session_id, offset, &text),
        session_id: session_id.to_string(),
        role: Role::Assistant,
        content: text,
        timestamp,
        source_offset: offset,
    })
}

fn hash_id(kind: &str, session_id: &str, offset: u64, text: &str) -> String {
    let prefix = super::super::safe_prefix(text, 100);
    blake3::hash(format!("{}:{}:{}:{}", kind, session_id, offset, prefix).as_bytes())
        .to_hex()
        .to_string()
}

fn extract_user_text(content: &[serde_json::Value]) -> String {
    let mut parts = Vec::new();
    for item in content {
        if item.get("type").and_then(|v| v.as_str()) != Some("input_text") {
            continue;
        }
        let text = item
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if text.is_empty() || text.starts_with("<environment_context>") {
            continue;
        }
        parts.push(text.to_string());
    }
    parts.join("\n").trim().to_string()
}

fn extract_assistant_text(content: &[serde_json::Value]) -> String {
    let mut parts = Vec::new();
    for item in content {
        if item.get("type").and_then(|v| v.as_str()) != Some("output_text") {
            continue;
        }
        let text = item
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if !text.is_empty() {
            parts.push(text.to_string());
        }
    }
    parts.join("\n").trim().to_string()
}
