use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::debug;

use super::Adapter;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct ContinueAdapter {
    base_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SessionIndex {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "workspaceDirectory")]
    workspace_directory: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SessionFile {
    #[serde(default)]
    history: Vec<HistoryItem>,
}

#[derive(Debug, Deserialize)]
struct HistoryItem {
    message: Option<ContinueMessage>,
}

#[derive(Debug, Deserialize)]
struct ContinueMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
    id: Option<String>,
}

impl Default for ContinueAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ContinueAdapter {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".continue")
            .join("sessions");
        Self { base_dir }
    }

    #[cfg(test)]
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn extract_text(content: &serde_json::Value) -> String {
        match content {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        }
    }
}

fn workspace_to_project(ws: &str) -> Option<String> {
    ws.strip_prefix("file://").map(|s| s.to_string()).or_else(|| {
        if ws.is_empty() {
            None
        } else {
            Some(ws.to_string())
        }
    })
}

impl Adapter for ContinueAdapter {
    fn name(&self) -> &str {
        "continue"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let index_path = self.base_dir.join("sessions.json");
        if !index_path.exists() {
            return Ok(Vec::new());
        }

        let index_content = fs::read_to_string(&index_path)
            .with_context(|| format!("failed to read {}", index_path.display()))?;
        let entries: Vec<SessionIndex> = match serde_json::from_str(&index_content) {
            Ok(e) => e,
            Err(e) => {
                debug!("continue: failed to parse sessions.json: {}", e);
                return Ok(Vec::new());
            }
        };

        let mut sessions = Vec::new();
        for entry in entries {
            let file_path = self.base_dir.join(format!("{}.json", entry.session_id));
            if !file_path.exists() {
                continue;
            }

            let mtime = fs::metadata(&file_path)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let project = entry
                .workspace_directory
                .as_deref()
                .and_then(workspace_to_project);

            sessions.push(SessionMeta {
                id: entry.session_id.clone(),
                source: "continue".to_string(),
                project_path: project,
                file_path: file_path.to_string_lossy().to_string(),
                last_offset: 0,
                mtime,
            });
        }

        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        let path = Path::new(&session.file_path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", session.file_path))?;

        let parsed: SessionFile = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                debug!("continue: failed to parse {}: {}", session.file_path, e);
                return Ok(Vec::new());
            }
        };

        let mut messages = Vec::new();

        for (i, item) in parsed.history.iter().enumerate() {
            let msg = match &item.message {
                Some(m) => m,
                None => continue,
            };

            let role_str = match msg.role.as_deref() {
                Some(r) => r,
                None => continue,
            };
            let role = match role_str {
                "user" | "human" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                "tool" => Role::Tool,
                _ => continue,
            };

            let text = match &msg.content {
                Some(c) => Self::extract_text(c),
                None => continue,
            };

            if text.trim().is_empty() {
                continue;
            }

            let id = msg
                .id
                .clone()
                .unwrap_or_else(|| {
                    blake3::hash(
                        format!("{}{}{}", session.id, i, super::safe_prefix(&text, 100))
                            .as_bytes(),
                    )
                    .to_hex()
                    .to_string()
                });

            messages.push(RawMessage {
                id,
                session_id: session.id.clone(),
                role,
                content: text,
                timestamp: None,
                source_offset: i as u64,
            });
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_continue_session() {
        let tmp = TempDir::new().unwrap();

        let index = r#"[{"sessionId":"test-001","title":"Test Chat","workspaceDirectory":"file:///tmp/project","dateCreated":"1700000000000"}]"#;
        fs::write(tmp.path().join("sessions.json"), index).unwrap();

        let session_data = r#"{
            "sessionId": "test-001",
            "title": "Test Chat",
            "workspaceDirectory": "file:///tmp/project",
            "history": [
                {
                    "message": {
                        "role": "user",
                        "content": [{"type": "text", "text": "hello continue"}],
                        "id": "msg-1"
                    }
                },
                {
                    "message": {
                        "role": "assistant",
                        "content": "Hi! How can I help?",
                        "id": "msg-2"
                    }
                }
            ]
        }"#;
        fs::write(tmp.path().join("test-001.json"), session_data).unwrap();

        let adapter = ContinueAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].project_path.as_deref(), Some("/tmp/project"));

        let messages = adapter.collect(&sessions[0]).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert!(messages[0].content.contains("hello continue"));
        assert_eq!(messages[1].role, Role::Assistant);
    }

    #[test]
    fn test_empty_sessions_dir() {
        let tmp = TempDir::new().unwrap();
        let adapter = ContinueAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        assert!(sessions.is_empty());
    }
}
