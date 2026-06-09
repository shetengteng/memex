//! `Adapter::collect` 主流程：按 composerId 拉对话消息体。
//!
//! 同时承担 bubble JSON 的内容抽取（text / richText / toolFormerData 三态）
//! 和 generic-title 过滤（避免"Conversation initiation"这类无意义占位）。

use anyhow::{Context, Result};
use rusqlite::params;
use tracing::debug;

use super::CursorSqliteAdapter;
use super::types::{
    BUBBLE_KEY_PREFIX, Bubble, COMPOSER_KEY_PREFIX, ComposerData, value_ref_to_string,
};
use crate::collector::safe_prefix;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub(super) fn collect_messages(
    adapter: &CursorSqliteAdapter,
    session: &SessionMeta,
) -> Result<Vec<RawMessage>> {
    let composer_id = session
        .id
        .strip_prefix("cursor-")
        .unwrap_or(&session.id)
        .to_string();

    let Some(conn) = adapter.open_readonly()? else {
        return Ok(Vec::new());
    };

    let composer_key = format!("{}{}", COMPOSER_KEY_PREFIX, composer_id);
    let composer_text: Option<String> = conn
        .query_row(
            "SELECT value FROM cursorDiskKV WHERE key = ?1",
            params![composer_key],
            |row| Ok(value_ref_to_string(row.get_ref(0)?)),
        )
        .ok()
        .flatten();
    let Some(composer_text) = composer_text else {
        return Ok(Vec::new());
    };
    let composer: ComposerData = serde_json::from_str(&composer_text)
        .with_context(|| format!("cursor[sqlite]: parse composer {composer_id}"))?;

    let headers = composer.headers.unwrap_or_default();
    let start = session.last_offset as usize;
    if start >= headers.len() {
        return Ok(Vec::new());
    }

    let mut messages = Vec::with_capacity(headers.len() - start);
    for (idx, header) in headers.iter().enumerate().skip(start) {
        let key = format!("{}{}:{}", BUBBLE_KEY_PREFIX, composer_id, header.bubble_id);
        let bubble_text: Option<String> = conn
            .query_row(
                "SELECT value FROM cursorDiskKV WHERE key = ?1",
                params![&key],
                |row| Ok(value_ref_to_string(row.get_ref(0)?)),
            )
            .ok()
            .flatten();
        let Some(bubble_text) = bubble_text else {
            continue;
        };
        let bubble: Bubble = match serde_json::from_str(&bubble_text) {
            Ok(b) => b,
            Err(e) => {
                debug!("cursor[sqlite]: skip malformed bubble {}: {}", key, e);
                continue;
            }
        };

        let type_id = bubble.type_.or(header.type_);
        let role = match type_id {
            Some(1) => Role::User,
            Some(2) => Role::Assistant,
            _ => continue,
        };

        let content = match bubble_content(&bubble) {
            Some(c) if !c.trim().is_empty() => c,
            _ => continue,
        };

        let offset_ix = (idx as u64) + 1;
        let id = blake3::hash(
            format!(
                "{}{}{}",
                session.id,
                header.bubble_id,
                safe_prefix(&content, 100)
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();

        messages.push(RawMessage {
            id,
            session_id: session.id.clone(),
            role,
            content,
            timestamp: None,
            source_offset: offset_ix,
        });
    }

    Ok(messages)
}

pub(super) fn is_generic_title(s: &str) -> bool {
    const GENERIC: &[&str] = &[
        "conversation initiation",
        "conversation start",
        "start the conversation",
        "start of the conversation",
        "new conversation",
        "开始对话",
        "新对话",
        "新的对话",
        "继续讨论",
        "prompts file discussion",
        "prompts from prompts.txt",
    ];
    let lower = s.to_lowercase();
    GENERIC.iter().any(|g| lower == *g)
}

fn bubble_content(bubble: &Bubble) -> Option<String> {
    if let Some(text) = bubble.text.as_ref()
        && !text.trim().is_empty()
    {
        return Some(text.clone());
    }
    if let Some(rich) = bubble.rich_text.as_ref()
        && !rich.trim().is_empty()
    {
        return Some(rich.clone());
    }
    if let Some(tool) = bubble.tool_former_data.as_ref() {
        let name = tool.name.as_deref().unwrap_or("tool");
        let mut parts = Vec::new();
        parts.push(format!("[tool: {}]", name));
        if let Some(args) = &tool.raw_args
            && !args.trim().is_empty()
        {
            parts.push(format!("args: {}", args));
        }
        if let Some(result) = &tool.result
            && !result.trim().is_empty()
        {
            parts.push(format!("result: {}", result));
        }
        if parts.len() > 1 {
            return Some(parts.join("\n"));
        }
    }
    None
}
