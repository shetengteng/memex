#[cfg(test)]
mod tests;

mod jsonl;
mod sqlite;

use std::path::PathBuf;

use anyhow::Result;

use super::Adapter;
use crate::storage::models::{RawMessage, SessionMeta};

pub use jsonl::CursorJsonlAdapter;
pub use sqlite::{CursorSqliteAdapter, CursorSqliteProbe};

/// Read backend for the Cursor adapter.
enum Backend {
    Sqlite(CursorSqliteAdapter),
    Jsonl(CursorJsonlAdapter),
}

pub struct CursorAdapter {
    backend: Backend,
}

impl Default for CursorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorAdapter {
    /// Default: read Cursor sessions directly from its globalStorage SQLite KV.
    pub fn new() -> Self {
        Self {
            backend: Backend::Sqlite(CursorSqliteAdapter::new()),
        }
    }

    /// Legacy JSONL mode (agent-transcripts directory). Kept for tests and
    /// for users who export Cursor sessions to disk.
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self {
            backend: Backend::Jsonl(CursorJsonlAdapter::with_base_dir(base_dir)),
        }
    }

    /// Explicit SQLite mode pointing at a custom `state.vscdb` (used by tests).
    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self {
            backend: Backend::Sqlite(CursorSqliteAdapter::with_db_path(db_path)),
        }
    }
}

impl Adapter for CursorAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        match &self.backend {
            Backend::Sqlite(a) => a.scan(),
            Backend::Jsonl(a) => a.scan(),
        }
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        match &self.backend {
            Backend::Sqlite(a) => a.collect(session),
            Backend::Jsonl(a) => a.collect(session),
        }
    }
}

/// Strip common Cursor workspace path prefixes (Users-<name>-Documents-, etc.).
/// Kept module-public so the JSONL backend can share it.
pub(crate) fn normalize_workspace_name(raw: &str) -> String {
    let mut name = raw.to_string();
    if let Some(idx) = name.find("-Documents-") {
        name = name[idx + "-Documents-".len()..].to_string();
    } else if let Some(idx) = name.find("-Library-Application-Support-Cursor-Workspaces-") {
        name = format!(
            "ws:{}",
            &name[idx + "-Library-Application-Support-Cursor-Workspaces-".len()..]
        );
    } else if let Some(start) = name.find("Users-") {
        if let Some(sep) = name[start + "Users-".len()..].find('-') {
            name = name[start + "Users-".len() + sep + 1..].to_string();
        }
    }
    name
}
