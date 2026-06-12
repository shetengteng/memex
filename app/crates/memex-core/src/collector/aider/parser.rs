//! `.aider.chat.history.md` 的解析逻辑。
//!
//! Aider 把每个会话写成 markdown 块，块与块之间用
//! `# aider chat started at <timestamp>` 分隔；块内消息约定：
//!   - `#### <user prompt>`  → user
//!   - `> <text>`            → tool（aider 输出，如 "Applied edit to ..."）
//!   - 其余非空行              → assistant
//!
//! 把扫描和解析独立到这里，让 `mod.rs` 只负责 trait 实现 + 文件发现。

use crate::storage::models::{RawMessage, Role};

/// 把 history 文件按会话切分。
/// 会话之间以匹配 `# aider chat started at <timestamp>` 的行为分隔符。
pub(super) fn split_sessions(content: &str) -> Vec<(String, String)> {
    let mut sessions = Vec::new();
    let mut start_ts = String::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut seg_start = 0;
    let mut has_start = false;

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("# aider chat started at ") {
            if has_start {
                let block = lines[seg_start..i].join("\n");
                sessions.push((start_ts.clone(), block));
            }
            start_ts = line
                .strip_prefix("# aider chat started at ")
                .unwrap_or("")
                .trim()
                .to_string();
            has_start = true;
            seg_start = i + 1;
        }
    }

    if has_start && seg_start < lines.len() {
        let block = lines[seg_start..].join("\n");
        sessions.push((start_ts, block));
    }

    sessions
}

/// 从单个会话块里解析出消息列表。
pub(super) fn parse_session_messages(session_id: &str, block: &str) -> Vec<RawMessage> {
    let mut messages = Vec::new();
    let mut current_role: Option<Role> = None;
    let mut current_content = String::new();
    let mut msg_index: usize = 0;

    let flush =
        |role: Role, content: &str, sid: &str, idx: &mut usize, out: &mut Vec<RawMessage>| {
            let text = content.trim();
            if text.is_empty() {
                return;
            }
            let id = blake3::hash(
                format!(
                    "{}{}{}",
                    sid,
                    *idx,
                    crate::collector::safe_prefix(text, 100)
                )
                .as_bytes(),
            )
            .to_hex()
            .to_string();
            out.push(RawMessage {
                id,
                session_id: sid.to_string(),
                role,
                content: text.to_string(),
                timestamp: None,
                source_offset: *idx as u64,
            });
            *idx += 1;
        };

    for line in block.lines() {
        if let Some(user_text) = line.strip_prefix("#### ") {
            if let Some(role) = current_role.take() {
                flush(
                    role,
                    &current_content,
                    session_id,
                    &mut msg_index,
                    &mut messages,
                );
                current_content.clear();
            }
            current_role = Some(Role::User);
            current_content.push_str(user_text);
            current_content.push('\n');
        } else if line.starts_with("> ") || line == ">" {
            if current_role != Some(Role::Tool) {
                if let Some(role) = current_role.take() {
                    flush(
                        role,
                        &current_content,
                        session_id,
                        &mut msg_index,
                        &mut messages,
                    );
                    current_content.clear();
                }
                current_role = Some(Role::Tool);
            }
            let text = line.strip_prefix("> ").unwrap_or("");
            current_content.push_str(text);
            current_content.push('\n');
        } else {
            if current_role == Some(Role::User) || current_role == Some(Role::Tool) {
                if let Some(role) = current_role.take() {
                    flush(
                        role,
                        &current_content,
                        session_id,
                        &mut msg_index,
                        &mut messages,
                    );
                    current_content.clear();
                }
                current_role = Some(Role::Assistant);
            } else if current_role.is_none() && !line.trim().is_empty() {
                current_role = Some(Role::Assistant);
            }
            if current_role.is_some() {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }
    }

    if let Some(role) = current_role {
        flush(
            role,
            &current_content,
            session_id,
            &mut msg_index,
            &mut messages,
        );
    }

    messages
}
