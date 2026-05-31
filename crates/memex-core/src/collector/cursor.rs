use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{debug, warn};

use super::Adapter;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct CursorAdapter {
    base_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CursorEntry {
    role: Option<String>,
    message: Option<CursorMessage>,
}

#[derive(Debug, Deserialize)]
struct CursorMessage {
    content: Option<serde_json::Value>,
}

impl CursorAdapter {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".cursor")
            .join("projects");
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
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file()
                && path.extension().is_some_and(|ext| ext == "jsonl")
                && path.to_string_lossy().contains("agent-transcripts")
            {
                files.push(path.to_path_buf());
            }
        }
        Ok(files)
    }

    fn extract_project_name(&self, file_path: &Path) -> Option<String> {
        let path_str = file_path.to_string_lossy();
        let base_str = self.base_dir.to_string_lossy();
        let relative = path_str.strip_prefix(base_str.as_ref())?.trim_start_matches('/');
        let workspace_part = relative.split('/').next()?;
        Some(
            workspace_part
                .replace("Users-TerrellShe-Documents-", "")
                .replace("Users-TerrellShe-", "")
                .replace("Library-Application-Support-Cursor-Workspaces-", "ws:")
        )
    }

    fn session_id_from_path(path: &Path) -> String {
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| blake3::hash(path.to_string_lossy().as_bytes()).to_hex().to_string())
    }
}

impl Adapter for CursorAdapter {
    fn name(&self) -> &str {
        "cursor"
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

            let session_id = Self::session_id_from_path(&file_path);
            let project_path = self.extract_project_name(&file_path);

            sessions.push(SessionMeta {
                id: session_id,
                source: "cursor".to_string(),
                project_path,
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

        let file = fs::File::open(path)
            .with_context(|| format!("failed to open {}", session.file_path))?;
        let file_size = file.metadata()?.len();

        if file_size <= session.last_offset {
            return Ok(Vec::new());
        }

        let mut reader = BufReader::new(file);
        if session.last_offset > 0 {
            reader.seek(SeekFrom::Start(session.last_offset))?;
        }

        let mut messages = Vec::new();
        let mut current_offset = session.last_offset;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    warn!("cursor: read error at offset {}: {}", current_offset, e);
                    break;
                }
            };
            current_offset += line.len() as u64 + 1;

            if line.trim().is_empty() {
                continue;
            }

            let entry: CursorEntry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(e) => {
                    debug!("cursor: skipping malformed JSON: {}", e);
                    continue;
                }
            };

            if let Some(msg) = convert_cursor_entry(&entry, &session.id, current_offset) {
                messages.push(msg);
            }
        }

        Ok(messages)
    }
}

fn convert_cursor_entry(entry: &CursorEntry, session_id: &str, offset: u64) -> Option<RawMessage> {
    let role_str = entry.role.as_deref()?;
    let role = match role_str {
        "user" | "human" => Role::User,
        "assistant" => Role::Assistant,
        "system" => Role::System,
        "tool" => Role::Tool,
        _ => return None,
    };

    let content = extract_content(&entry.message)?;
    if content.trim().is_empty() {
        return None;
    }

    let id = blake3::hash(
        format!("{}{}{}", session_id, offset, super::safe_prefix(&content, 100)).as_bytes(),
    )
    .to_hex()
    .to_string();

    Some(RawMessage {
        id,
        session_id: session_id.to_string(),
        role,
        content,
        timestamp: None,
        source_offset: offset,
    })
}

fn extract_content(message: &Option<CursorMessage>) -> Option<String> {
    let msg = message.as_ref()?;
    let content_val = msg.content.as_ref()?;

    match content_val {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    parts.push(text.to_string());
                }
            }
            if parts.is_empty() { None } else { Some(parts.join("\n")) }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_cursor_jsonl() {
        let tmp = TempDir::new().unwrap();
        let content = r#"{"role":"user","message":{"content":[{"type":"text","text":"hello cursor"}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"hi from cursor assistant"}]}}
"#;
        let dir = tmp.path().join("proj/agent-transcripts/uuid1");
        fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("uuid1.jsonl");
        fs::write(&file_path, content).unwrap();

        let adapter = CursorAdapter::with_base_dir(tmp.path().to_path_buf());
        let session = SessionMeta {
            id: "uuid1".to_string(),
            source: "cursor".to_string(),
            project_path: Some("proj".to_string()),
            file_path: file_path.to_string_lossy().to_string(),
            last_offset: 0,
            mtime: 0,
        };

        let messages = adapter.collect(&session).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert!(messages[0].content.contains("hello cursor"));
        assert_eq!(messages[1].role, Role::Assistant);
    }

    #[test]
    fn test_scan_discovers_transcripts() {
        let tmp = TempDir::new().unwrap();
        let dir1 = tmp.path().join("proj-a/agent-transcripts/s1");
        let dir2 = tmp.path().join("proj-b/agent-transcripts/s2");
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();
        fs::write(dir1.join("s1.jsonl"), r#"{"role":"user","message":{"content":"hi"}}"#).unwrap();
        fs::write(dir2.join("s2.jsonl"), r#"{"role":"user","message":{"content":"hey"}}"#).unwrap();

        let adapter = CursorAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 2);
    }
}
