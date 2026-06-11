//! Schema migrations driven by [`rusqlite_migration`].
//!
//! The library tracks the applied version via `PRAGMA user_version` instead
//! of a hand-rolled `schema_version` table. We expose a single `v1`
//! migration that:
//!
//! 1. Drops every legacy object that an older Memex install may have left
//!    behind (`schema_version` table, dangling triggers, partial tables).
//!    The user has explicitly accepted **data loss on schema upgrade** in
//!    `design/20260609-01-Memex-Rust改造TODO.md` (P1-5), so old rows are
//!    sacrificed in exchange for a much simpler upgrade story.
//! 2. Creates the latest [`schema::SCHEMA_SQL`] in one shot.
//!
//! Fresh installs hit step 1 as a no-op (every `DROP IF EXISTS` is silent)
//! and then run step 2 normally.  Old installs (any pre-`user_version`
//! Memex on disk) get reset to the current shape; the next ingest cycle
//! rebuilds session / message / chunk rows from the source files.
//!
//! Future schema changes append a new `M::up(...)` entry below; they run
//! on top of the post-baseline state and bump `user_version` accordingly.

use std::sync::OnceLock;

use rusqlite_migration::{M, Migrations};

use super::schema::SCHEMA_SQL;

/// Order matters: drop FTS triggers and the FTS shadow virtual table
/// *before* the `chunks` base table, then drop child tables before their
/// parents (FK references). `IF EXISTS` keeps the statement safe on a
/// fresh DB.
const DROP_LEGACY_SQL: &str = "
DROP TRIGGER IF EXISTS chunks_ai;
DROP TRIGGER IF EXISTS chunks_ad;
DROP TRIGGER IF EXISTS chunks_au;
DROP TABLE   IF EXISTS chunks_fts;
DROP TABLE   IF EXISTS thread_sessions;
DROP TABLE   IF EXISTS threads;
DROP TABLE   IF EXISTS aggregate_summaries;
DROP TABLE   IF EXISTS summaries;
DROP TABLE   IF EXISTS redactions;
DROP TABLE   IF EXISTS chunks;
DROP TABLE   IF EXISTS messages;
DROP TABLE   IF EXISTS sessions;
DROP TABLE   IF EXISTS sources;
DROP TABLE   IF EXISTS access_log;
DROP TABLE   IF EXISTS metrics;
DROP TABLE   IF EXISTS kv;
DROP TABLE   IF EXISTS llm_providers;
DROP TABLE   IF EXISTS schema_version;
";

/// Baseline SQL (DROP legacy + CREATE latest), cached as a single
/// `&'static str` so the `M::up(&'u str)` borrow lives long enough.
///
/// Stored in a `OnceLock` to avoid leaking a fresh `String` on every
/// `Db::open()` (Box::leak would also work but compounds across reopens
/// in long-running test sessions).
fn baseline_sql() -> &'static str {
    static SQL: OnceLock<String> = OnceLock::new();
    SQL.get_or_init(|| format!("{}\n{}", DROP_LEGACY_SQL, SCHEMA_SQL))
}

/// v2: 给已有库追加 `mcp_call_log` 表 + 两个索引。
/// 全 `IF NOT EXISTS`，对 fresh install 是 no-op（baseline 已经包含）。
const ADD_MCP_CALL_LOG_SQL: &str = "
CREATE TABLE IF NOT EXISTS mcp_call_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    success INTEGER NOT NULL DEFAULT 1,
    error_message TEXT
);
CREATE INDEX IF NOT EXISTS idx_mcp_call_log_occurred_desc
    ON mcp_call_log(occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_mcp_call_log_tool
    ON mcp_call_log(tool_name);
";

/// v3: `mcp_call_log` 增列 `arguments_json` / `result_json`，把 MCP 调用 payload
/// 落库给 UI 详情卡用。`ALTER TABLE ADD COLUMN` 在 SQLite 里是 O(1) 元数据改动，
/// 不会重写数据页；对历史行新列为 NULL，符合 `Option<String>` 语义。
///
/// 用 `ALTER TABLE` 而不是 baseline 重建，是因为存量用户的 mcp_call_log 行不能丢
/// （UI 上是 \"24h 聚合 / 准实时事件流\" 的唯一数据源）。
const ADD_MCP_CALL_PAYLOAD_SQL: &str = "
ALTER TABLE mcp_call_log ADD COLUMN arguments_json TEXT;
ALTER TABLE mcp_call_log ADD COLUMN result_json TEXT;
";

/// Build the migration set.
pub(super) fn build_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(baseline_sql()),
        M::up(ADD_MCP_CALL_LOG_SQL),
        M::up(ADD_MCP_CALL_PAYLOAD_SQL),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Catches obvious authoring mistakes (e.g. malformed SQL strings)
    /// without opening a real DB. Recommended by the upstream README.
    #[test]
    fn migrations_validate() {
        assert!(
            build_migrations().validate().is_ok(),
            "migration set is malformed; check DROP_LEGACY_SQL / SCHEMA_SQL"
        );
    }
}
