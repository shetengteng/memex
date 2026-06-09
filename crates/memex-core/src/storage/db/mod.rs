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
mod threads;

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

pub use providers::LlmProviderRow;
pub use sessions::{MessageRow, NewSession, SessionDetail, SessionRow};
pub use summaries::{AggregateSummaryRow, AggregateSummaryUpsert, SummaryRow, SummaryUpsert};
pub use threads::{ThreadDetail, ThreadDraft, ThreadRow};

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
            conn.execute("UPDATE schema_version SET version = ?1", params![2u32])?;
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
            conn.execute("UPDATE schema_version SET version = ?1", params![3u32])?;
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
            conn.execute("UPDATE schema_version SET version = ?1", params![4u32])?;
        }
        if from < 5 {
            // v5: 给 summaries 加 message_count_at_creation 字段，
            // 用于「摘要过期检测」（方案 A）。
            // 存量摘要把该字段回填为「当前 session 的 message_count」，
            // 视为「与当前一致 / 未过期」，避免迁移直接触发全量重摘要。
            let has_col: bool = conn
                .prepare("PRAGMA table_info(summaries)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .any(|name| name.as_deref() == Ok("message_count_at_creation"));
            if !has_col {
                conn.execute_batch(
                    "ALTER TABLE summaries
                     ADD COLUMN message_count_at_creation INTEGER NOT NULL DEFAULT 0;",
                )?;
                // 回填：从 sessions.message_count 拷贝过来。
                conn.execute(
                    "UPDATE summaries
                     SET message_count_at_creation = (
                         SELECT message_count FROM sessions WHERE sessions.id = summaries.session_id
                     )
                     WHERE message_count_at_creation = 0",
                    [],
                )?;
            }
            conn.execute("UPDATE schema_version SET version = ?1", params![5u32])?;
        }
        if from < 6 {
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_chunks_has_summary
                    ON chunks(id) WHERE summary IS NOT NULL;",
            )?;
            conn.execute("UPDATE schema_version SET version = ?1", params![6u32])?;
        }
        if from < 7 {
            // v7：sessions 表新增 intent 列。存量行该列为 NULL，等下次
            // 摘要重生成（用户主动「重新生成」或 message_count 增长触发过期）
            // 时由 LLM 输出填充，不做强制全量回填以免高 LLM 成本。
            let has_col: bool = conn
                .prepare("PRAGMA table_info(sessions)")?
                .query_map([], |row| row.get::<_, String>(1))?
                .any(|name| name.as_deref() == Ok("intent"));
            if !has_col {
                conn.execute_batch("ALTER TABLE sessions ADD COLUMN intent TEXT;")?;
            }
            conn.execute("UPDATE schema_version SET version = ?1", params![7u32])?;
        }
        if from < 8 {
            // v8：在 messages(timestamp) 上加局部索引。
            // 之前 workload_report 的 heatmap 通路按 timestamp 范围扫描，
            // 在 30w+ messages 真实库上 30 天查询要 6+s，导致 Insights 趋势 tab
            // 加载超时显示空白。局部索引（WHERE timestamp IS NOT NULL）只覆盖
            // 有 timestamp 的行（claude_code / codex / opencode），cursor 没
            // timestamp 的 messages 完全不进索引，体积可控。
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_messages_timestamp
                    ON messages(timestamp) WHERE timestamp IS NOT NULL;",
            )?;
            conn.execute("UPDATE schema_version SET version = ?1", params![8u32])?;
        }
        if from < 9 {
            // v9：补齐两个本来应该早就在的热路径索引。
            //   (a) idx_chunks_has_summary：v6 migration 里加过，但 SCHEMA_SQL
            //       一直没加，导致从 v6 之后新装的库 / v6→v8 跳级升级的库
            //       都没有这个索引。后果是「摘要进度百分比」展示
            //       (chunks_with_summary_count) 在 70w+ chunks 真实库上要 15+ 秒。
            //   (b) idx_messages_content_dedup：ingest 每条消息 dedup 查询
            //       `WHERE content_hash = ? AND session_id = ?` 没有复合索引，
            //       退化为按 session 全扫比对 hash，大 session 上一次 50+ ms。
            // 这两个索引现在都已经写入 SCHEMA_SQL；这里 migration 只是给老库
            // 兜底补建。`CREATE INDEX IF NOT EXISTS` 是幂等的。
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_chunks_has_summary
                    ON chunks(id) WHERE summary IS NOT NULL;
                 CREATE INDEX IF NOT EXISTS idx_messages_content_dedup
                    ON messages(content_hash, session_id);",
            )?;
            conn.execute("UPDATE schema_version SET version = ?1", params![9u32])?;
        }
        if from < 10 {
            // v10：新增 threads + thread_sessions N:N 中间表，用于 L5
            // 「主题线索」聚类（LLM 把多个 session 聚合成线索）。
            // 这是纯新增表，老库迁移只创建结构，不回填数据；下一次手动
            // 触发 regenerate_threads 或者 try_l5_thread_clustering 跑完
            // 才会有内容。SCHEMA_SQL 里也写了相同 DDL，新装库直接拿到。
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS threads (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    summary TEXT NOT NULL DEFAULT '',
                    session_count INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(name)
                );
                 CREATE TABLE IF NOT EXISTS thread_sessions (
                    thread_id INTEGER NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
                    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                    confidence REAL NOT NULL DEFAULT 1.0,
                    created_at TEXT NOT NULL,
                    PRIMARY KEY (thread_id, session_id)
                );
                 CREATE INDEX IF NOT EXISTS idx_thread_sessions_session
                    ON thread_sessions(session_id);
                 CREATE INDEX IF NOT EXISTS idx_threads_updated_at
                    ON threads(updated_at DESC);",
            )?;
            conn.execute("UPDATE schema_version SET version = ?1", params![10u32])?;
        }
        Ok(())
    }
}
