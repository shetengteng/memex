use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::debug;

use super::Adapter;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct OpenCodeAdapter {
    base_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct OpenCodeSession {
    messages: Option<Vec<OpenCodeMessage>>,
}

#[derive(Debug, Deserialize)]
struct OpenCodeMessage {
    role: Option<String>,
    content: Option<String>,
}

impl Default for OpenCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".opencode")
            .join("sessions");
        Self { base_dir }
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn discover_session_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if !self.base_dir.exists() {
            return Ok(files);
        }

        for entry in walkdir::WalkDir::new(&self.base_dir)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                files.push(path.to_path_buf());
            }
        }
        Ok(files)
    }

    fn session_id_from_path(path: &Path) -> String {
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                blake3::hash(path.to_string_lossy().as_bytes())
                    .to_hex()
                    .to_string()
            })
    }
}

impl Adapter for OpenCodeAdapter {
    fn name(&self) -> &str {
        "opencode"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let files = self.discover_session_files()?;
        let mut sessions = Vec::new();

        for file_path in files {
            let meta = match fs::metadata(&file_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            sessions.push(SessionMeta {
                id: Self::session_id_from_path(&file_path),
                source: "opencode".to_string(),
                project_path: None,
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

        let parsed: OpenCodeSession = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                debug!("opencode: failed to parse {}: {}", session.file_path, e);
                return Ok(Vec::new());
            }
        };

        let raw_messages = parsed.messages.unwrap_or_default();
        let mut messages = Vec::new();

        for (i, msg) in raw_messages.iter().enumerate() {
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
                Some(c) if !c.trim().is_empty() => c.clone(),
                _ => continue,
            };

            let id = blake3::hash(
                format!("{}{}{}", session.id, i, super::safe_prefix(&text, 100)).as_bytes(),
            )
            .to_hex()
            .to_string();

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
    fn test_parse_opencode_session() {
        let tmp = TempDir::new().unwrap();
        let content = r#"{"messages":[{"role":"user","content":"hello opencode"},{"role":"assistant","content":"response"}]}"#;
        let file_path = tmp.path().join("session1.json");
        fs::write(&file_path, content).unwrap();

        let adapter = OpenCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let session = SessionMeta {
            id: "session1".to_string(),
            source: "opencode".to_string(),
            project_path: None,
            file_path: file_path.to_string_lossy().to_string(),
            last_offset: 0,
            mtime: 0,
        };

        let messages = adapter.collect(&session).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "hello opencode");
    }

    #[test]
    fn test_missing_fields_handled() {
        let tmp = TempDir::new().unwrap();
        let content = r#"{"messages":[{"role":"user","content":"good"},{"role":null,"content":"skip this"}]}"#;
        let file_path = tmp.path().join("s2.json");
        fs::write(&file_path, content).unwrap();

        let adapter = OpenCodeAdapter::with_base_dir(tmp.path().to_path_buf());
        let session = SessionMeta {
            id: "s2".to_string(),
            source: "opencode".to_string(),
            project_path: None,
            file_path: file_path.to_string_lossy().to_string(),
            last_offset: 0,
            mtime: 0,
        };

        let messages = adapter.collect(&session).unwrap();
        assert_eq!(messages.len(), 1);
    }
}
