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
mod migrations;
pub mod providers;
mod schema;
mod sessions;
mod sources;
mod summaries;
#[cfg(test)]
mod tests;
mod threads;

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::clock::{ArcClock, SystemClock};

pub use providers::LlmProviderRow;
pub use sessions::{MessageRow, NewSession, SessionDetail, SessionListFilter, SessionRow};
pub use summaries::{AggregateSummaryRow, AggregateSummaryUpsert, SummaryRow, SummaryUpsert};
pub use threads::{ThreadDetail, ThreadDraft, ThreadRow};

pub struct Db {
    pub(crate) conn: Mutex<Connection>,
    pub(crate) clock: ArcClock,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_clock(path, Arc::new(SystemClock))
    }

    /// 与 [`Self::open`] 同语义，但允许调用方注入自定义 [`Clock`](crate::clock::Clock)
    /// 实现。生产代码不必使用；测试中需要确定性时间戳时改走这个入口。
    pub fn open_with_clock(path: &Path, clock: ArcClock) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database: {}", path.display()))?;
        let db = Self {
            conn: Mutex::new(conn),
            clock,
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::open_in_memory_with_clock(Arc::new(SystemClock))
    }

    /// 内存数据库 + 注入自定义 [`Clock`](crate::clock::Clock)，仅给单元测试使用。
    pub fn open_in_memory_with_clock(clock: ArcClock) -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Mutex::new(conn),
            clock,
        };
        db.init_schema()?;
        Ok(db)
    }

    /// 注入的 clock "现在"。Db 内部所有需要 `chrono::Utc::now()` 的位置
    /// 一律走这个 helper，让 `FrozenClock` 注入下时间戳完全确定。
    pub(crate) fn now_utc(&self) -> DateTime<Utc> {
        self.clock.now_utc()
    }

    fn init_schema(&self) -> Result<()> {
        let mut conn = self.conn.lock();
        // Keep PRAGMAs outside the migration transaction (best practice
        // per rusqlite_migration docs).
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrations::build_migrations()
            .to_latest(&mut conn)
            .context("failed to apply schema migrations")?;
        Ok(())
    }
}
