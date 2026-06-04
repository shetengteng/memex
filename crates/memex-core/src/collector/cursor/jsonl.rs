use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{debug, warn};

use crate::collector::{Adapter, safe_prefix};
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct CursorJsonlAdapter {
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

impl CursorJsonlAdapter {
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn discover_session_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if !self.base_dir.exists() {
            debug!(
                "cursor: base_dir does not exist ({}); skipping",
                self.base_dir.display()
            );
            return Ok(files);
        }

        match fs::read_dir(&self.base_dir) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                warn!(
                    "cursor: permission denied reading {}.\n  \
                     macOS likely needs Full Disk Access for the terminal running `memex`.\n  \
                     Grant it via System Settings → Privacy & Security → Full Disk Access,\n  \
                     then re-run `memex ingest`. Skipping cursor adapter for now.",
                    self.base_dir.display()
                );
                return Ok(files);
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("cursor: failed to read {}", self.base_dir.display())
                });
            }
        }

        let mut perm_warned = false;
        for entry in walkdir::WalkDir::new(&self.base_dir).into_iter() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    if e.io_error()
                        .is_some_and(|io| io.kind() == std::io::ErrorKind::PermissionDenied)
                        && !perm_warned
                    {
                        warn!(
                            "cursor: walkdir hit permission denied at {:?}; \
                             some Cursor session files may be skipped. \
                             Grant Full Disk Access to fix.",
                            e.path()
                        );
                        perm_warned = true;
                    } else {
                        debug!("cursor: walkdir error: {}", e);
                    }
                    continue;
                }
            };
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
        let relative = file_path.strip_prefix(&self.base_dir).ok()?;
        let workspace_part = relative.components().next()?;
        let raw = workspace_part.as_os_str().to_string_lossy().to_string();
        Some(super::normalize_workspace_name(&raw))
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

impl Adapter for CursorJsonlAdapter {
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

            let created_secs = meta
                .created()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            sessions.push(SessionMeta {
                id: session_id,
                source: "cursor".to_string(),
                project_path,
                file_path: file_path.to_string_lossy().to_string(),
                last_offset: 0,
                mtime,
                created_secs,
                title: None,
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
        format!("{}{}{}", session_id, offset, safe_prefix(&content, 100)).as_bytes(),
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
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}
