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

/// Cursor adapter 的读取后端。
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
    /// 默认实现：直接从 Cursor 的 globalStorage SQLite KV 里读会话。
    pub fn new() -> Self {
        Self {
            backend: Backend::Sqlite(CursorSqliteAdapter::new()),
        }
    }

    /// 旧版 JSONL 模式（agent-transcripts 目录）。保留是为了测试以及那些
    /// 把 Cursor 会话导出到本地文件的用户。
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self {
            backend: Backend::Jsonl(CursorJsonlAdapter::with_base_dir(base_dir)),
        }
    }

    /// 显式 SQLite 模式，指向自定义的 `state.vscdb`（测试用）。
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

/// 去掉 Cursor workspace 路径里的公共前缀（Users-<name>-Documents- 等）。
/// 保留 module-public，方便 JSONL 后端共用。
pub(crate) fn normalize_workspace_name(raw: &str) -> String {
    let mut name = raw.to_string();
    if let Some(idx) = name.find("-Documents-") {
        name = name[idx + "-Documents-".len()..].to_string();
    } else if let Some(idx) = name.find("-Library-Application-Support-Cursor-Workspaces-") {
        name = format!(
            "ws:{}",
            &name[idx + "-Library-Application-Support-Cursor-Workspaces-".len()..]
        );
    } else if let Some(start) = name.find("Users-")
        && let Some(sep) = name[start + "Users-".len()..].find('-')
    {
        name = name[start + "Users-".len() + sep + 1..].to_string();
    }
    name
}
