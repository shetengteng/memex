//! Cursor SQLite adapter —— 从 `state.vscdb` 里读 composer + bubble 对话。
//!
//! 子模块拆分：
//! - [`types`]：跨模块共享的反序列化 DTO + 字节解码
//! - [`scan`]：[`Adapter::scan`] 主体（带 composerHeaders enrichment）
//! - [`collect`]：[`Adapter::collect`] 主体 + bubble content / generic-title
//! - [`probe`]：[`CursorSqliteAdapter::probe`]（doctor / settings 用）
//! - [`project_path`]：从 raw JSON 推断 project_path 的纯函数集合

mod collect;
mod probe;
mod project_path;
mod scan;
mod types;

use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::{debug, warn};

use crate::collector::Adapter;
use crate::storage::models::{RawMessage, SessionMeta};

pub use probe::CursorSqliteProbe;

use collect::is_generic_title;

pub struct CursorSqliteAdapter {
    db_path: PathBuf,
}

impl CursorSqliteAdapter {
    pub fn new() -> Self {
        let db_path = dirs::home_dir()
            .expect("INVARIANT: home directory must be resolvable")
            .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb");
        Self { db_path }
    }

    pub fn with_db_path(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    /// 共享给 `scan` / `collect` 的只读连接打开器。FDA 拒绝时返回 `Ok(None)`，
    /// 让上游优雅降级（跳过 cursor 这个 adapter，不影响其余 collector）。
    pub(super) fn open_readonly(&self) -> Result<Option<rusqlite::Connection>> {
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
        match rusqlite::Connection::open_with_flags(
            &uri,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_URI
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => Ok(Some(c)),
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
                Err(e).with_context(|| {
                    format!("cursor[sqlite]: failed to open {}", self.db_path.display())
                })
            }
        }
    }
}

impl Default for CursorSqliteAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for CursorSqliteAdapter {
    fn name(&self) -> &str {
        "cursor"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        scan::scan_sessions(self)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        collect::collect_messages(self, session)
    }
}
