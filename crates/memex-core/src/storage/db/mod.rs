//! SQLite handle for Memex. The single `Db` value owns the `Mutex<Connection>`
//! and is shared across collector / processor / retriever / daemon paths.
//!
//! Logic is split across siblings to keep every file under the 300-line cap:
//!   * `schema`   — DDL (`SCHEMA_SQL`) and the version constant.
//!   * `sources`  — adapter file offsets / mtimes (incremental scan state).
//!   * `sessions` — session CRUD + the `SessionRow` / `SessionDetail` shapes.
//!   * `messages` — dedup-aware insert with per-session counters.
//!   * `chunks`   — chunk inserts and FTS5 search.
//!   * `kv`       — generic config KV and the redaction audit log.

mod chunks;
mod kv;
mod messages;
mod schema;
mod sessions;
mod sources;
mod summaries;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

pub use sessions::{MessageRow, SessionDetail, SessionRow};
pub use summaries::{AggregateSummaryRow, SummaryRow};

pub struct Db {
    pub(crate) conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database: {}", path.display()))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);",
        )?;

        let version: Option<u32> = conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok();

        if version.is_none() {
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                params![schema::SCHEMA_VERSION],
            )?;
        }

        conn.execute_batch(schema::SCHEMA_SQL)?;

        if let Some(v) = version {
            Self::run_migrations(&conn, v)?;
        }

        Ok(())
    }

    fn run_migrations(conn: &Connection, from: u32) -> Result<()> {
        if from < 2 {
            let has_summary: bool = conn
                .prepare("PRAGMA table_info(chunks)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .any(|name| name.as_deref() == Ok("summary"));
            if !has_summary {
                conn.execute_batch("ALTER TABLE chunks ADD COLUMN summary TEXT;")?;
            }
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS aggregate_summaries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    scope_type TEXT NOT NULL,
                    scope_key TEXT NOT NULL,
                    title TEXT,
                    summary TEXT NOT NULL,
                    topics_json TEXT NOT NULL DEFAULT '[]',
                    decisions_json TEXT NOT NULL DEFAULT '[]',
                    session_count INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    UNIQUE(scope_type, scope_key)
                );",
            )?;
            conn.execute(
                "UPDATE schema_version SET version = ?1",
                params![2u32],
            )?;
        }
        if from < 3 {
            // v3: add indexes for popup list_sessions_paged hot path
            // (the SCHEMA_SQL block above re-runs CREATE INDEX IF NOT EXISTS,
            // so this just bumps the version for existing DBs.)
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_messages_session_role_offset
                    ON messages(session_id, role, source_offset);
                 CREATE INDEX IF NOT EXISTS idx_summaries_session_level
                    ON summaries(session_id, level);
                 CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
                    ON sessions(updated_at DESC);",
            )?;
            conn.execute(
                "UPDATE schema_version SET version = ?1",
                params![3u32],
            )?;
        }
        Ok(())
    }
}
