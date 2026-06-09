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

/// Build the migration set.
pub(super) fn build_migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(baseline_sql())])
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
