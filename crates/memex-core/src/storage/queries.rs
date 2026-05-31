//! Read-only queries that are not part of the per-table CRUD modules:
//!   * `doctor` payload (schema version, fts health, adapter status, counts)
//!   * `access_log` write helper
//!   * Session-id prefix lookup (used by `memex session show <prefix>`)
//!
//! Per-table CRUD lives under `db/{sources,sessions,messages,chunks,kv}.rs`.

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::db::Db;

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub db_exists: bool,
    pub schema_version: Option<u32>,
    pub session_count: u64,
    pub message_count: u64,
    pub chunk_count: u64,
    pub source_count: u64,
    pub fts_ok: bool,
    pub adapters: Vec<AdapterStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterStatus {
    pub name: String,
    pub file_count: u64,
    pub last_scan: Option<String>,
}

impl Db {
    pub fn write_access_log(
        &self,
        query: &str,
        result_count: usize,
        latency_ms: u64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO access_log (query, result_count, latency_ms, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                query,
                result_count as i64,
                latency_ms as i64,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn source_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn schema_version(&self) -> Result<Option<u32>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok())
    }

    pub fn fts_health_check(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM chunks_fts", [], |row| {
            row.get::<_, i64>(0)
        })
        .is_ok()
    }

    pub fn adapter_statuses(&self) -> Result<Vec<AdapterStatus>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
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

    pub fn find_session_by_prefix(&self, prefix: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_log() {
        let db = Db::open_in_memory().unwrap();
        db.write_access_log("redis", 5, 42).unwrap();
    }

    #[test]
    fn test_find_session_by_prefix() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session("abc-12345", "claude_code", None, "/f.jsonl")
            .unwrap();
        assert_eq!(
            db.find_session_by_prefix("abc-1").unwrap().unwrap(),
            "abc-12345"
        );
        assert!(db.find_session_by_prefix("zzz").unwrap().is_none());
    }

    #[test]
    fn test_fts_health() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.fts_health_check());
    }

    #[test]
    fn test_doctor_queries() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.schema_version().unwrap().is_some());
        assert_eq!(db.source_count().unwrap(), 0);
        assert!(db.adapter_statuses().unwrap().is_empty());
    }
}
