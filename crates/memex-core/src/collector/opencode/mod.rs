//! OpenCode adapter —— 从 `~/.local/share/opencode/opencode.db` (SQLite) 把
//! session / message / part 三张表 join 起来形成 RawMessage 流。
//!
//! 拆分：把 `#[cfg(test)] mod tests` 抽到 sibling `tests.rs`，主文件只保留
//! Adapter 接线代码 + `is_opencode_placeholder_title` 这个会被测试直接调用
//! 的小过滤器。

#[cfg(test)]
mod tests;

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::Deserialize;
use tracing::debug;

use super::Adapter;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct OpenCodeAdapter {
    db_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct MessageData {
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PartData {
    #[serde(rename = "type")]
    part_type: Option<String>,
    text: Option<String>,
}

impl Default for OpenCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("cannot determine home directory");
        let xdg_data = home.join(".local/share");
        let db_path = if xdg_data.join("opencode/opencode.db").exists() {
            xdg_data.join("opencode/opencode.db")
        } else {
            dirs::data_dir()
                .unwrap_or(xdg_data)
                .join("opencode")
                .join("opencode.db")
        };
        Self { db_path }
    }

    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

impl Adapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        if !self.db_path.exists() {
            return Ok(Vec::new());
        }

        let conn = rusqlite::Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("failed to open opencode db: {:?}", self.db_path))?;

        let mut stmt = conn.prepare(
            "SELECT s.id, s.title, s.directory, s.time_created, s.time_updated
             FROM session s
             ORDER BY s.time_updated DESC",
        )?;

        let sessions = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let directory: String = row.get(2)?;
                let time_created: i64 = row.get(3)?;
                let time_updated: i64 = row.get(4)?;
                let mtime_secs = (time_updated / 1000) as u64;
                let created_secs = if time_created > 0 {
                    (time_created / 1000) as u64
                } else {
                    0
                };

                Ok(SessionMeta {
                    id,
                    source: "opencode".to_string(),
                    project_path: Some(directory),
                    file_path: self.db_path.to_string_lossy().to_string(),
                    last_offset: 0,
                    mtime: mtime_secs,
                    created_secs,
                    // opencode 在 session 创建时给一个形如 "New session - 2026-01-23T..."
                    // 的占位 title。等效于 cursor 的 "Conversation initiation"：
                    // 没有任何语义、也不该挤掉 first_user_message 在 popup/列表里的显示。
                    // 这里直接视为空，让 UI fallback 走 first_user_message。
                    title: Some(title)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty() && !is_opencode_placeholder_title(s)),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        if !self.db_path.exists() {
            return Ok(Vec::new());
        }

        let conn = rusqlite::Connection::open_with_flags(
            &self.db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        let mut stmt = conn.prepare(
            "SELECT m.id, m.data, m.time_created FROM message m
             WHERE m.session_id = ?1
             ORDER BY m.time_created ASC",
        )?;

        let msg_rows: Vec<(String, String, i64)> = stmt
            .query_map(params![session.id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut messages = Vec::new();

        for (msg_id, data_json, time_created) in &msg_rows {
            let msg_data: MessageData = match serde_json::from_str(data_json) {
                Ok(d) => d,
                Err(e) => {
                    debug!("opencode: failed to parse message {}: {}", msg_id, e);
                    continue;
                }
            };

            let role = match msg_data.role.as_deref() {
                Some("user" | "human") => Role::User,
                Some("assistant") => Role::Assistant,
                Some("system") => Role::System,
                Some("tool") => Role::Tool,
                _ => continue,
            };

            let mut part_stmt = conn.prepare(
                "SELECT data FROM part
                 WHERE message_id = ?1
                 ORDER BY time_created ASC",
            )?;

            let parts: Vec<String> = part_stmt
                .query_map(params![msg_id], |row| row.get::<_, String>(0))?
                .filter_map(|r| r.ok())
                .collect();

            let mut text_parts = Vec::new();
            for part_json in &parts {
                if let Ok(pd) = serde_json::from_str::<PartData>(part_json)
                    && pd.part_type.as_deref() == Some("text")
                    && let Some(t) = pd.text
                    && !t.trim().is_empty()
                {
                    text_parts.push(t);
                }
            }

            if text_parts.is_empty() {
                continue;
            }

            let content = text_parts.join("\n");
            let ts_millis = *time_created;
            let ts = DateTime::<Utc>::from_timestamp(
                ts_millis / 1000,
                ((ts_millis % 1000) * 1_000_000) as u32,
            );

            let id = blake3::hash(
                format!(
                    "{}{}{}",
                    session.id,
                    msg_id,
                    super::safe_prefix(&content, 100)
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
                timestamp: ts,
                source_offset: messages.len() as u64,
            });
        }

        Ok(messages)
    }
}

/// opencode 在 session 创建瞬间会写入形如 `New session - 2026-01-23T08:45:35.508Z`
/// 的 placeholder title（用户从未给会话起名时的默认值）。这种 title 不携带语义，
/// 应该让 popup / dashboard 的 fallback 链跳过它、显示 `first_user_message` 才有用。
fn is_opencode_placeholder_title(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.starts_with("new session - ") || lower == "new session"
}
