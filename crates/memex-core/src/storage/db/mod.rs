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
pub use summaries::SummaryRow;

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
        Ok(())
    }
}
