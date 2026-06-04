//! Memex 的 SQLite 句柄。单一的 `Db` 值持有 `Mutex<Connection>`，
//! collector / processor / retriever / daemon 各路径都共用它。
//!
//! 逻辑拆到平级模块里，保证每个文件不超过 300 行：
//!   * `schema`   —— DDL（`SCHEMA_SQL`）和版本号常量。
//!   * `sources`  —— adapter 的文件 offset / mtime（增量扫描状态）。
//!   * `sessions` —— 会话的 CRUD，以及 `SessionRow` / `SessionDetail` 数据结构。
//!   * `messages` —— 带去重逻辑的插入，附带按会话维度的计数。
//!   * `chunks`   —— chunk 写入和 FTS5 搜索。
//!   * `kv`       —— 通用配置 KV 和脱敏审计日志。

mod chunks;
mod kv;
mod messages;
pub mod providers;
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

pub use providers::LlmProviderRow;
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
            // v3：为 popup 的 list_sessions_paged 热路径加索引
            //（上面的 SCHEMA_SQL 也会跑 CREATE INDEX IF NOT EXISTS，
            //  这里只是给老库升一下版本号。）
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
        if from < 4 {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS llm_providers (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    kind TEXT NOT NULL,
                    base_url TEXT NOT NULL,
                    model TEXT NOT NULL DEFAULT '',
                    api_key TEXT NOT NULL DEFAULT '',
                    enabled INTEGER NOT NULL DEFAULT 1,
                    is_default INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL DEFAULT 'untested',
                    latency_ms INTEGER,
                    updated_at TEXT NOT NULL
                );",
            )?;
            conn.execute(
                "UPDATE schema_version SET version = ?1",
                params![4u32],
            )?;
        }
        Ok(())
    }
}
