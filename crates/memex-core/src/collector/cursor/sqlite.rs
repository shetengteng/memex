use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::params;
use rusqlite::types::ValueRef;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::collector::{Adapter, safe_prefix};
use crate::storage::models::{RawMessage, Role, SessionMeta};

const COMPOSER_KEY_PREFIX: &str = "composerData:";
const BUBBLE_KEY_PREFIX: &str = "bubbleId:";

pub struct CursorSqliteAdapter {
    db_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ComposerData {
    #[serde(rename = "composerId")]
    composer_id: Option<String>,
    name: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<i64>,
    #[serde(rename = "lastUpdatedAt")]
    last_updated_at: Option<i64>,
    #[serde(rename = "fullConversationHeadersOnly")]
    headers: Option<Vec<ConversationHeader>>,
}

#[derive(Debug, Deserialize)]
struct ConversationHeader {
    #[serde(rename = "bubbleId")]
    bubble_id: String,
    #[serde(default, rename = "type")]
    type_: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct Bubble {
    #[serde(rename = "type")]
    type_: Option<i64>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "richText")]
    rich_text: Option<String>,
    #[serde(default, rename = "toolFormerData")]
    tool_former_data: Option<ToolFormerData>,
}

#[derive(Debug, Deserialize)]
struct ToolFormerData {
    name: Option<String>,
    #[serde(default)]
    result: Option<String>,
    #[serde(default, rename = "rawArgs")]
    raw_args: Option<String>,
}

impl CursorSqliteAdapter {
    pub fn new() -> Self {
        let db_path = dirs::home_dir()
            .expect("cannot determine home directory")
            .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb");
        Self { db_path }
    }

    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    fn open_readonly(&self) -> Result<Option<rusqlite::Connection>> {
        if !self.db_path.exists() {
            debug!(
                "cursor[sqlite]: db not found at {}; skipping",
                self.db_path.display()
            );
            return Ok(None);
        }
        let uri = format!(
            "file:{}?mode=ro&immutable=0",
            self.db_path.to_string_lossy()
        );
        let conn = match rusqlite::Connection::open_with_flags(
            &uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("unable to open")
                    || msg.contains("authorization denied")
                    || msg.contains("permission")
                {
                    warn!(
                        "cursor[sqlite]: cannot open {} ({msg}).\n  \
                         macOS likely needs Full Disk Access for the terminal running `memex`.\n  \
                         Grant it via System Settings → Privacy & Security → Full Disk Access,\n  \
                         then re-run `memex ingest`. Skipping cursor adapter for now.",
                        self.db_path.display()
                    );
                    return Ok(None);
                }
                return Err(e).with_context(|| {
                    format!("cursor[sqlite]: failed to open {}", self.db_path.display())
                });
            }
        };
        Ok(Some(conn))
    }
}

impl Default for CursorSqliteAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Cursor 的 `cursorDiskKV.value` 列声明类型是 BLOB，
/// 但实际写入既可能是 TEXT JSON（新版 Cursor，绝大多数 row）
/// 也可能是 BLOB 字节（老 fixture / 二进制 cache）。
/// 用 ValueRef 手动区分，避免 rusqlite 的严格类型检查报
/// "Invalid column type Text/Blob at index ... name: value"。
fn value_ref_to_string(value: ValueRef<'_>) -> Option<String> {
    match value {
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
            if bytes.is_empty() {
                None
            } else {
                std::str::from_utf8(bytes).ok().map(|s| s.to_string())
            }
        }
        ValueRef::Null => None,
        _ => None,
    }
}

/// 给 `memex doctor` 和 menubar 设置页用的轻量健康探测。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum CursorSqliteProbe {
    Ok { composer_count: i64, db_path: String },
    NotFound { db_path: String },
    PermissionDenied { db_path: String, message: String },
    Error { db_path: String, message: String },
}

impl CursorSqliteAdapter {
    pub fn probe(&self) -> CursorSqliteProbe {
        if !self.db_path.exists() {
            return CursorSqliteProbe::NotFound {
                db_path: self.db_path.to_string_lossy().to_string(),
            };
        }
        let uri = format!(
            "file:{}?mode=ro&immutable=0",
            self.db_path.to_string_lossy()
        );
        match rusqlite::Connection::open_with_flags(
            &uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(conn) => match conn.query_row(
                "SELECT COUNT(*) FROM cursorDiskKV WHERE key LIKE 'composerData:%'",
                [],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(n) => CursorSqliteProbe::Ok {
                    composer_count: n,
                    db_path: self.db_path.to_string_lossy().to_string(),
                },
                Err(e) => CursorSqliteProbe::Error {
                    db_path: self.db_path.to_string_lossy().to_string(),
                    message: e.to_string(),
                },
            },
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("unable to open")
                    || msg.contains("authorization")
                    || msg.contains("permission")
                {
                    CursorSqliteProbe::PermissionDenied {
                        db_path: self.db_path.to_string_lossy().to_string(),
                        message: msg,
                    }
                } else {
                    CursorSqliteProbe::Error {
                        db_path: self.db_path.to_string_lossy().to_string(),
                        message: msg,
                    }
                }
            }
        }
    }
}

impl Adapter for CursorSqliteAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let Some(conn) = self.open_readonly()? else {
            return Ok(Vec::new());
        };

        let mut stmt = conn
            .prepare("SELECT key, value FROM cursorDiskKV WHERE key LIKE ?1")
            .context("cursor[sqlite]: prepare composerData query failed")?;
        let pattern = format!("{}%", COMPOSER_KEY_PREFIX);
        let rows = stmt
            .query_map(params![pattern], |row| {
                let key: String = row.get(0)?;
                let value = value_ref_to_string(row.get_ref(1)?);
                Ok((key, value))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut sessions = Vec::with_capacity(rows.len());
        for (key, value) in rows {
            let text = match value {
                Some(s) => s,
                None => continue,
            };
            let composer: ComposerData = match serde_json::from_str(&text) {
                Ok(c) => c,
                Err(e) => {
                    debug!("cursor[sqlite]: skip malformed composer {}: {}", key, e);
                    continue;
                }
            };
            let composer_id = composer
                .composer_id
                .clone()
                .or_else(|| key.strip_prefix(COMPOSER_KEY_PREFIX).map(String::from))
                .unwrap_or_default();
            if composer_id.is_empty() {
                continue;
            }

            let mtime_ms = composer
                .last_updated_at
                .or(composer.created_at)
                .unwrap_or(0);
            let mtime = if mtime_ms > 0 {
                (mtime_ms / 1000) as u64
            } else {
                0
            };
            let created_ms = composer.created_at.unwrap_or(0);
            let created_secs = if created_ms > 0 { (created_ms / 1000) as u64 } else { 0 };

            sessions.push(SessionMeta {
                id: format!("cursor-{}", composer_id),
                source: "cursor".to_string(),
                project_path: composer.name.clone().filter(|s| !s.is_empty()),
                file_path: self.db_path.to_string_lossy().to_string(),
                last_offset: 0,
                mtime,
                created_secs,
            });
        }
        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        let composer_id = session
            .id
            .strip_prefix("cursor-")
            .unwrap_or(&session.id)
            .to_string();

        let Some(conn) = self.open_readonly()? else {
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

            let content = bubble_content(&bubble);
            let content = match content {
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
}

fn bubble_content(bubble: &Bubble) -> Option<String> {
    if let Some(text) = bubble.text.as_ref() {
        if !text.trim().is_empty() {
            return Some(text.clone());
        }
    }
    if let Some(rich) = bubble.rich_text.as_ref() {
        if !rich.trim().is_empty() {
            return Some(rich.clone());
        }
    }
    if let Some(tool) = bubble.tool_former_data.as_ref() {
        let name = tool.name.as_deref().unwrap_or("tool");
        let mut parts = Vec::new();
        parts.push(format!("[tool: {}]", name));
        if let Some(args) = &tool.raw_args {
            if !args.trim().is_empty() {
                parts.push(format!("args: {}", args));
            }
        }
        if let Some(result) = &tool.result {
            if !result.trim().is_empty() {
                parts.push(format!("result: {}", result));
            }
        }
        if parts.len() > 1 {
            return Some(parts.join("\n"));
        }
    }
    None
}
