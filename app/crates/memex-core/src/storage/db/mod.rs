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

    /// 主动把 WAL 中的写入 checkpoint 回主库并截断 -wal 文件。
    ///
    /// 用途：daemon 在每次 ingest 完成后调用一次，把 -wal 收敛到 0，避免
    /// 长跑 daemon 累积几百 MB WAL、MCP 冷启动重放变慢。TRUNCATE 模式比
    /// PASSIVE 更激进——若有其它连接持锁 checkpoint 会跳过，但 pragma 本身
    /// 仍返回成功，下一轮再试即可。
    ///
    /// # Errors
    ///
    /// 底层 SQLite `PRAGMA wal_checkpoint` 语句执行失败（DB 已关闭、IO 错误
    /// 等）时返回错误。「有其它连接持锁导致本次没截断」不算错误。
    pub fn wal_checkpoint_truncate(&self) -> Result<()> {
        let conn = self.conn.lock();
        // 单独 pragma_query_value 返回三列（busy / log / checkpointed），我们不关心
        // 具体值，忽略即可；execute_batch 不需要 rowset。
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .context("wal_checkpoint(TRUNCATE) failed")?;
        Ok(())
    }

    fn init_schema(&self) -> Result<()> {
        let mut conn = self.conn.lock();
        // Keep PRAGMAs outside the migration transaction (best practice
        // per rusqlite_migration docs).
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        // WAL 上限 64MB。之前遇到过 daemon 长期跑、-wal 涨到几百 MB 的场景，
        // MCP 侧新建连接时要重放全部 WAL，冷启动被拖到十几秒。设置 journal_size_limit
        // 后 SQLite 会在 checkpoint 时把超出部分截断回收。
        conn.execute_batch("PRAGMA journal_size_limit = 67108864;")?;
        // busy_timeout：让 SQLite 在遇到 SQLITE_BUSY 时自动 sleep + 重试，最多
        // 等 30 秒。
        //
        // 真实场景：用户「清空全部数据」时，maintenance::system_reset_all 走
        //   shutdown daemon → sleep 300ms → reset_all（fs 删除）→ Db::open
        //   （fresh 跑 migrations）
        // 但 shutdown 只关 daemon HTTP server / watcher 这条主链；前端定时器
        // 触发的 IPC handler（mcp_recent_calls / list_notifications 等）在 reset
        // 窗口里会临时 `Db::open` 自己一份 Connection 跑查询，与 reset 端的
        // migration transaction 抢锁，必现 `database is locked` (rusqlite_migration
        // baseline 的 DROP TABLE / CREATE TABLE 整个事务直接 fail)。
        //
        // busy_timeout 是 SQLite 处理这种瞬时锁竞争的标准方案：每个连接独立
        // 设置，跟 WAL 协作良好，写者撞 busy 时 sleep 一小段再试。30s 给得保守，
        // reset 实际窗口 100~500ms 即可松开。
        conn.execute_batch("PRAGMA busy_timeout = 30000;")?;
        migrations::build_migrations()
            .to_latest(&mut conn)
            .context("failed to apply schema migrations")?;
        Ok(())
    }
}
