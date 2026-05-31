//! Codex Desktop adapter — reads `~/.codex/session_index.jsonl` to discover
//! session ids, then locates the full message stream at
//! `~/.codex/sessions/YYYY/MM/DD/rollout-<datetime>-<session-id>.jsonl`.
//!
//! Verified against tars-ai-butler `tars/adapters/codex.py`:
//! - Each rollout JSONL contains entries dispatched by the top-level `type`:
//!     - `session_meta`: payload.cwd → project path
//!     - `response_item`: payload.role + payload.content (array of
//!       `{type: input_text | output_text, text}`) for user/assistant messages
//!     - `event_msg`: payload.type=`last_agent_message` → final assistant signal
//! - User messages whose first `input_text` starts with
//!   `<environment_context>` are auto-injected and must be filtered out.
//! - Timestamps end with `Z` and need RFC3339 with `+00:00`.

use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::{debug, warn};

use super::Adapter;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct CodexAdapter {
    base_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct IndexEntry {
    id: Option<String>,
    updated_at: Option<String>,
    thread_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SessionEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    timestamp: Option<String>,
    payload: Option<serde_json::Value>,
}

impl CodexAdapter {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".codex");
        Self { base_dir }
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn index_path(&self) -> PathBuf {
        self.base_dir.join("session_index.jsonl")
    }

    fn sessions_root(&self) -> PathBuf {
        self.base_dir.join("sessions")
    }

    /// Locate the JSONL file for a session id. tars uses the `updated_at` to
    /// build a date-prefixed path first (fast); falls back to a recursive scan.
    fn find_session_file(&self, session_id: &str, updated_at: Option<&str>) -> Option<PathBuf> {
        let sessions_root = self.sessions_root();
        if !sessions_root.exists() {
            return None;
        }

        if let Some(ts) = updated_at {
            if let Ok(dt) = DateTime::parse_from_rfc3339(&ts.replace('Z', "+00:00")) {
                let date_dir = sessions_root
                    .join(format!("{:04}", dt.year()))
                    .join(format!("{:02}", dt.month()))
                    .join(format!("{:02}", dt.day()));
                if let Some(p) = scan_dir_for_session(&date_dir, session_id) {
                    return Some(p);
                }
            }
        }

        for entry in walkdir::WalkDir::new(&sessions_root)
            .max_depth(5)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file()
                && path.extension().is_some_and(|ext| ext == "jsonl")
                && path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().contains(session_id))
            {
                return Some(path.to_path_buf());
            }
        }
        None
    }
}

fn scan_dir_for_session(dir: &Path, session_id: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let p = entry.path();
        if p.is_file()
            && p.extension().is_some_and(|ext| ext == "jsonl")
            && p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains(session_id))
        {
            Some(p)
        } else {
            None
        }
    })
}

trait DateAccessors {
    fn year(&self) -> i32;
    fn month(&self) -> u32;
    fn day(&self) -> u32;
}

impl DateAccessors for chrono::DateTime<chrono::FixedOffset> {
    fn year(&self) -> i32 {
        chrono::Datelike::year(self)
    }
    fn month(&self) -> u32 {
        chrono::Datelike::month(self)
    }
    fn day(&self) -> u32 {
        chrono::Datelike::day(self)
    }
}

impl Adapter for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let mut sessions = Vec::new();
        let index = self.index_path();
        if !index.exists() {
            return Ok(sessions);
        }

        let file = match fs::File::open(&index) {
            Ok(f) => f,
            Err(e) => {
                debug!("codex: cannot open {} ({})", index.display(), e);
                return Ok(sessions);
            }
        };
        let reader = BufReader::new(file);

        for line in reader.lines().map_while(Result::ok) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let entry: IndexEntry = match serde_json::from_str(trimmed) {
                Ok(e) => e,
                Err(e) => {
                    debug!("codex: bad index line: {}", e);
                    continue;
                }
            };
            let session_id = match entry.id.filter(|s| !s.is_empty()) {
                Some(id) => id,
                None => continue,
            };
            let session_file = match self.find_session_file(&session_id, entry.updated_at.as_deref()) {
                Some(p) => p,
                None => {
                    debug!("codex: session file missing for {}", session_id);
                    continue;
                }
            };

            let meta = match fs::metadata(&session_file) {
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
                id: session_id,
                source: "codex".to_string(),
                project_path: entry.thread_name,
                file_path: session_file.to_string_lossy().to_string(),
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
        let mut session_cwd: Option<String> = None;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    warn!("codex: read error at offset {}: {}", current_offset, e);
                    break;
                }
            };
            current_offset += line.len() as u64 + 1;

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let entry: SessionEntry = match serde_json::from_str(trimmed) {
                Ok(e) => e,
                Err(e) => {
                    debug!("codex: skipping malformed JSON: {}", e);
                    continue;
                }
            };

            let entry_type = entry.entry_type.as_deref().unwrap_or("");
            let payload = match &entry.payload {
                Some(p) => p,
                None => continue,
            };
            let timestamp = entry
                .timestamp
                .as_deref()
                .and_then(|ts| DateTime::parse_from_rfc3339(&ts.replace('Z', "+00:00")).ok())
                .map(|dt| dt.with_timezone(&Utc));

            match entry_type {
                "session_meta" => {
                    if session_cwd.is_none() {
                        if let Some(cwd) = payload.get("cwd").and_then(|v| v.as_str()) {
                            session_cwd = Some(cwd.to_string());
                        }
                    }
                }
                "response_item" => {
                    if let Some(msg) =
                        build_message_from_response(payload, &session.id, current_offset, timestamp)
                    {
                        messages.push(msg);
                    }
                }
                "event_msg" => {
                    if payload.get("type").and_then(|v| v.as_str()) == Some("last_agent_message") {
                        if let Some(text) = payload
                            .get("last_agent_message")
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                        {
                            let id = blake3::hash(
                                format!(
                                    "codex-event:{}:{}:{}",
                                    session.id,
                                    current_offset,
                                    super::safe_prefix(&text, 100)
                                )
                                .as_bytes(),
                            )
                            .to_hex()
                            .to_string();
                            messages.push(RawMessage {
                                id,
                                session_id: session.id.clone(),
                                role: Role::Assistant,
                                content: text,
                                timestamp,
                                source_offset: current_offset,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(messages)
    }
}

fn build_message_from_response(
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

    let id = blake3::hash(
        format!(
            "codex:{}:{}:{}",
            session_id,
            offset,
            super::safe_prefix(&text, 100)
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string();

    Some(RawMessage {
        id,
        session_id: session_id.to_string(),
        role,
        content: text,
        timestamp,
        source_offset: offset,
    })
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
        if text.is_empty() {
            continue;
        }
        if text.starts_with("<environment_context>") {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_index(base: &Path, entries: &[(&str, &str, &str)]) {
        let path = base.join("session_index.jsonl");
        let mut content = String::new();
        for (id, ts, name) in entries {
            content.push_str(
                &serde_json::json!({
                    "id": id,
                    "updated_at": ts,
                    "thread_name": name,
                })
                .to_string(),
            );
            content.push('\n');
        }
        fs::write(path, content).unwrap();
    }

    fn write_session(base: &Path, ts: &str, session_id: &str, body: &str) -> PathBuf {
        let dt = DateTime::parse_from_rfc3339(&ts.replace('Z', "+00:00")).unwrap();
        let dir = base
            .join("sessions")
            .join(format!("{:04}", dt.year()))
            .join(format!("{:02}", dt.month()))
            .join(format!("{:02}", dt.day()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(format!("rollout-{}-{}.jsonl", "20260301T000000", session_id));
        fs::write(&file, body).unwrap();
        file
    }

    #[test]
    fn test_scan_uses_session_index() {
        let tmp = TempDir::new().unwrap();
        write_index(tmp.path(), &[("sess-1", "2026-03-01T10:00:00Z", "demo thread")]);
        write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-1", "");

        let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "sess-1");
        assert_eq!(sessions[0].project_path.as_deref(), Some("demo thread"));
    }

    #[test]
    fn test_collect_parses_response_items() {
        let tmp = TempDir::new().unwrap();
        let body = r#"{"type":"session_meta","timestamp":"2026-03-01T10:00:00Z","payload":{"cwd":"/Users/x/proj"}}
{"type":"response_item","timestamp":"2026-03-01T10:00:01Z","payload":{"role":"user","content":[{"type":"input_text","text":"hello codex"}]}}
{"type":"response_item","timestamp":"2026-03-01T10:00:02Z","payload":{"role":"assistant","content":[{"type":"output_text","text":"hi from assistant"}]}}
"#;
        write_index(tmp.path(), &[("sess-collect", "2026-03-01T10:00:00Z", "thread")]);
        write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-collect", body);

        let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        let messages = adapter.collect(&sessions[0]).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].content, "hello codex");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].content, "hi from assistant");
    }

    #[test]
    fn test_environment_context_filtered() {
        let tmp = TempDir::new().unwrap();
        let body = r#"{"type":"response_item","timestamp":"2026-03-01T10:00:01Z","payload":{"role":"user","content":[{"type":"input_text","text":"<environment_context>OS=mac</environment_context>"},{"type":"input_text","text":"actual user prompt"}]}}
"#;
        write_index(tmp.path(), &[("sess-env", "2026-03-01T10:00:00Z", "t")]);
        write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-env", body);

        let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        let messages = adapter.collect(&sessions[0]).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "actual user prompt");
    }

    #[test]
    fn test_event_msg_last_agent_message_captured() {
        let tmp = TempDir::new().unwrap();
        let body = r#"{"type":"event_msg","timestamp":"2026-03-01T10:01:00Z","payload":{"type":"last_agent_message","last_agent_message":"Done. Next: deploy"}}
"#;
        write_index(tmp.path(), &[("sess-evt", "2026-03-01T10:00:00Z", "t")]);
        write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-evt", body);

        let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        let messages = adapter.collect(&sessions[0]).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, Role::Assistant);
        assert!(messages[0].content.contains("deploy"));
    }

    #[test]
    fn test_missing_session_index_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        assert!(sessions.is_empty());
    }
}
