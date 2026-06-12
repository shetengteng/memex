//! Doctor / health-check queries used by the Settings → System tab,
//! the CLI `memex doctor` command, and the IPC `doctor` invoke.
//!
//! Everything in this file is short — a few row scans + `PRAGMA`
//! reads — and lands on the storage layer that callers already hold
//! a `Db` for, so we hang the helpers off `impl Db` rather than
//! taking a raw `Connection`.

use anyhow::Result;
use rusqlite::params;

use super::AdapterStatus;
use crate::storage::db::Db;

impl Db {
    /// Append one row to `access_log` for telemetry. `latency_ms` is
    /// measured by the caller; this helper only persists.
    pub fn write_access_log(
        &self,
        query: &str,
        result_count: usize,
        latency_ms: u64,
    ) -> Result<()> {
        let now = self.now_utc().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO access_log (query, result_count, latency_ms, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                query,
                result_count as i64,
                latency_ms as i64,
                now
            ],
        )?;
        Ok(())
    }

    /// Number of distinct `sources` rows (one per adapter-tracked
    /// file).
    pub fn source_count(&self) -> Result<u64> {
        let conn = self.conn.lock();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Returns the applied schema version as tracked by SQLite's
    /// `PRAGMA user_version` (managed by `rusqlite_migration`).
    ///
    /// Returns `Ok(None)` only for a never-initialized DB (user_version
    /// = 0), which in practice should never reach this code path
    /// because [`Db::open`] always runs `to_latest()` first. The
    /// `Option` is kept for backwards-compat with the `DoctorReport`
    /// IPC contract (frontend renders "未知" on `null`).
    pub fn schema_version(&self) -> Result<Option<u32>> {
        let conn = self.conn.lock();
        let v: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        Ok(if v == 0 { None } else { Some(v) })
    }

    /// True if `chunks_fts` accepts a basic count, i.e. the FTS5
    /// shadow table + triggers are intact.
    pub fn fts_health_check(&self) -> bool {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM chunks_fts", [], |row| {
            row.get::<_, i64>(0)
        })
        .is_ok()
    }

    /// Per-adapter file-count + last-scan timestamp, used by the
    /// doctor view.
    pub fn adapter_statuses(&self) -> Result<Vec<AdapterStatus>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT adapter, COUNT(*) as cnt, MAX(last_scan) as ls
             FROM sources GROUP BY adapter",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(AdapterStatus {
                    name: row.get(0)?,
                    file_count: row.get(1)?,
                    last_scan: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Resolve a short session-id prefix (e.g. the 8-char form the
    /// CLI accepts) into the full session id, preferring the most
    /// recently active session on collisions.
    pub fn find_session_by_prefix(&self, prefix: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let pattern = format!("{}%", prefix);
        Ok(conn
            .query_row(
                "SELECT id FROM sessions WHERE id LIKE ?1 ORDER BY updated_at DESC LIMIT 1",
                params![pattern],
                |row| row.get(0),
            )
            .ok())
    }
}
