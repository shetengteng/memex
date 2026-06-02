//! Codex Desktop adapter —— 通过读取 `~/.codex/session_index.jsonl` 发现
//! 所有 session id，然后从
//! `~/.codex/sessions/YYYY/MM/DD/rollout-<datetime>-<session-id>.jsonl`
//! 定位到完整的消息流。
//!
//! 已对照 tars-ai-butler `tars/adapters/codex.py` 验证：
//! - `session_meta` payload.cwd → 项目路径
//! - `response_item` payload（role + content 数组）→ user/assistant 消息，
//!   并过滤掉 Codex 注入的 `<environment_context>` 块
//! - `event_msg` payload.type=`last_agent_message` → 助手收尾信号
//! - 时间戳以 `Z` 结尾，需要做 RFC3339 归一化为 `+00:00`

mod discover;
mod parser;

#[cfg(test)]
mod tests;

use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::{debug, warn};

use super::Adapter;
use crate::storage::models::{RawMessage, SessionMeta};

use discover::{IndexEntry, find_session_file};
use parser::{SessionEntry, build_message_from_event, build_message_from_response};

pub struct CodexAdapter {
    base_dir: PathBuf,
}

impl CodexAdapter {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".codex");
        Self { base_dir }
    }

    #[cfg(test)]
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn index_path(&self) -> PathBuf {
        self.base_dir.join("session_index.jsonl")
    }

    fn sessions_root(&self) -> PathBuf {
        self.base_dir.join("sessions")
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CodexAdapter {
    fn name(&self) -> &str {
        "codex"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let index = self.index_path();
        if !index.exists() {
            return Ok(Vec::new());
        }
        let file = match fs::File::open(&index) {
            Ok(f) => f,
            Err(e) => {
                debug!("codex: cannot open {} ({})", index.display(), e);
                return Ok(Vec::new());
            }
        };
        let reader = BufReader::new(file);
        let sessions_root = self.sessions_root();
        let mut sessions = Vec::new();

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
            let session_file =
                match find_session_file(&sessions_root, &session_id, entry.updated_at.as_deref()) {
                    Some(p) => p,
                    None => continue,
                };
            let mtime = mtime_secs(&session_file).unwrap_or(0);
            let created_secs = created_secs(&session_file).unwrap_or(0);

            sessions.push(SessionMeta {
                id: session_id,
                source: "codex".to_string(),
                project_path: entry.thread_name,
                file_path: session_file.to_string_lossy().to_string(),
                last_offset: 0,
                mtime,
                created_secs,
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
                    warn!("codex: read error at offset {}: {}", current_offset, e);
                    break;
                }
            };
            current_offset += line.len() as u64 + 1;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(msg) = parse_entry(trimmed, &session.id, current_offset) {
                messages.push(msg);
            }
        }

        Ok(messages)
    }
}

fn parse_entry(line: &str, session_id: &str, offset: u64) -> Option<RawMessage> {
    let entry: SessionEntry = match serde_json::from_str(line) {
        Ok(e) => e,
        Err(e) => {
            debug!("codex: skipping malformed JSON: {}", e);
            return None;
        }
    };
    let payload = entry.payload.as_ref()?;
    let timestamp = entry
        .timestamp
        .as_deref()
        .and_then(|ts| DateTime::parse_from_rfc3339(&ts.replace('Z', "+00:00")).ok())
        .map(|dt| dt.with_timezone(&Utc));

    match entry.entry_type.as_deref().unwrap_or("") {
        "response_item" => build_message_from_response(payload, session_id, offset, timestamp),
        "event_msg" => build_message_from_event(payload, session_id, offset, timestamp),
        _ => None,
    }
}

fn mtime_secs(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

fn created_secs(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .created()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}
