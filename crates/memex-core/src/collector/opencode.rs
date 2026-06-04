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
                let created_secs = if time_created > 0 { (time_created / 1000) as u64 } else { 0 };

                Ok(SessionMeta {
                    id,
                    source: "opencode".to_string(),
                    project_path: Some(directory),
                    file_path: self.db_path.to_string_lossy().to_string(),
                    last_offset: 0,
                    mtime: mtime_secs,
                    created_secs,
                    title: Some(title).filter(|s| !s.is_empty()),
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
                if let Ok(pd) = serde_json::from_str::<PartData>(part_json) {
                    if pd.part_type.as_deref() == Some("text") {
                        if let Some(t) = pd.text {
                            if !t.trim().is_empty() {
                                text_parts.push(t);
                            }
                        }
                    }
                }
            }

            if text_parts.is_empty() {
                continue;
            }

            let content = text_parts.join("\n");
            let ts_millis = *time_created;
            let ts = DateTime::<Utc>::from_timestamp(ts_millis / 1000, ((ts_millis % 1000) * 1_000_000) as u32);

            let id = blake3::hash(
                format!("{}{}{}", session.id, msg_id, super::safe_prefix(&content, 100)).as_bytes(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_db(dir: &std::path::Path) -> PathBuf {
        let db_path = dir.join("opencode.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE project (id TEXT PRIMARY KEY, name TEXT);
             INSERT INTO project VALUES ('proj1', 'test-project');
             CREATE TABLE session (
                 id TEXT PRIMARY KEY, project_id TEXT, parent_id TEXT,
                 slug TEXT NOT NULL, directory TEXT NOT NULL, title TEXT NOT NULL,
                 version TEXT NOT NULL, share_url TEXT, summary_additions INTEGER,
                 summary_deletions INTEGER, summary_files INTEGER, summary_diffs TEXT,
                 revert TEXT, permission TEXT,
                 time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
                 time_compacting INTEGER, time_archived INTEGER
             );
             INSERT INTO session VALUES ('ses_001', 'proj1', NULL, 'test', '/tmp/proj', 'Test Session', '1', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 1000000, 2000000, NULL, NULL);
             CREATE TABLE message (
                 id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
                 time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
                 data TEXT NOT NULL
             );
             INSERT INTO message VALUES ('msg_001', 'ses_001', 1000000, 1000000, '{\"role\":\"user\"}');
             INSERT INTO message VALUES ('msg_002', 'ses_001', 1001000, 1001000, '{\"role\":\"assistant\"}');
             CREATE TABLE part (
                 id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL,
                 time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
                 data TEXT NOT NULL
             );
             INSERT INTO part VALUES ('prt_001', 'msg_001', 'ses_001', 1000000, 1000000, '{\"type\":\"text\",\"text\":\"hello opencode\"}');
             INSERT INTO part VALUES ('prt_002', 'msg_002', 'ses_001', 1001000, 1001000, '{\"type\":\"text\",\"text\":\"response from opencode\"}');",
        )
        .unwrap();
        db_path
    }

    #[test]
    fn test_scan_opencode_sessions() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());
        let adapter = OpenCodeAdapter::with_db_path(db_path);
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "ses_001");
        assert_eq!(sessions[0].project_path, Some("/tmp/proj".to_string()));
    }

    #[test]
    fn test_collect_opencode_messages() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());
        let adapter = OpenCodeAdapter::with_db_path(db_path);
        let sessions = adapter.scan().unwrap();
        let messages = adapter.collect(&sessions[0]).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "hello opencode");
        assert_eq!(messages[1].content, "response from opencode");
    }

    #[test]
    fn test_missing_db() {
        let adapter = OpenCodeAdapter::with_db_path(PathBuf::from("/nonexistent/opencode.db"));
        let sessions = adapter.scan().unwrap();
        assert!(sessions.is_empty());
    }
}
