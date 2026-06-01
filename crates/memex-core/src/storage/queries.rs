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

#[derive(Debug, Clone, Serialize)]
pub struct TimelineEntry {
    pub date: String,
    pub adapter: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsBreakdown {
    pub by_adapter: std::collections::BTreeMap<String, i64>,
    pub by_project: std::collections::BTreeMap<String, i64>,
    pub recent_7d_sessions: i64,
    pub recent_7d_messages: i64,
    pub recent_30d_sessions: i64,
    pub recent_30d_messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub project_path: String,
    pub name: String,
    pub session_count: i64,
    pub message_count: i64,
    pub last_title: Option<String>,
    pub last_updated: String,
    pub by_adapter: std::collections::BTreeMap<String, i64>,
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

    pub fn timeline(&self, days: u32) -> Result<Vec<TimelineEntry>> {
        let conn = self.conn.lock().unwrap();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT DATE(updated_at) as d, source, COUNT(*) as cnt,
                    SUM(message_count) as msgs
             FROM sessions WHERE updated_at >= ?1
             GROUP BY d, source ORDER BY d DESC",
        )?;
        let rows = stmt
            .query_map(params![cutoff], |row| {
                Ok(TimelineEntry {
                    date: row.get(0)?,
                    adapter: row.get(1)?,
                    sessions: row.get(2)?,
                    messages: row.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn stats_breakdown(&self) -> Result<StatsBreakdown> {
        let conn = self.conn.lock().unwrap();
        let mut by_adapter = std::collections::BTreeMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT source, COUNT(*) FROM sessions GROUP BY source",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in rows.flatten() {
                by_adapter.insert(r.0, r.1);
            }
        }
        let mut by_project = std::collections::BTreeMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT project_path, COUNT(*) FROM sessions
                 WHERE project_path IS NOT NULL GROUP BY project_path
                 ORDER BY COUNT(*) DESC LIMIT 20",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in rows.flatten() {
                by_project.insert(r.0, r.1);
            }
        }
        let now = chrono::Utc::now();
        let d7 = (now - chrono::Duration::days(7)).to_rfc3339();
        let d30 = (now - chrono::Duration::days(30)).to_rfc3339();
        let recent_7d: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(message_count),0) FROM sessions WHERE updated_at >= ?1",
            params![d7],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or((0, 0));
        let recent_30d: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(message_count),0) FROM sessions WHERE updated_at >= ?1",
            params![d30],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or((0, 0));
        Ok(StatsBreakdown {
            by_adapter,
            by_project,
            recent_7d_sessions: recent_7d.0,
            recent_7d_messages: recent_7d.1,
            recent_30d_sessions: recent_30d.0,
            recent_30d_messages: recent_30d.1,
        })
    }

    pub fn list_project_summaries(&self) -> Result<Vec<ProjectSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT project_path,
                    COUNT(*) as cnt,
                    COALESCE(SUM(message_count), 0) as msgs,
                    MAX(updated_at) as last_upd
             FROM sessions
             WHERE project_path IS NOT NULL
             GROUP BY project_path
             ORDER BY last_upd DESC",
        )?;
        let base_rows: Vec<(String, i64, i64, String)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut results = Vec::with_capacity(base_rows.len());
        for (path, session_count, message_count, last_updated) in base_rows {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();

            let last_title: Option<String> = conn
                .query_row(
                    "SELECT title FROM sessions
                     WHERE project_path = ?1 AND title IS NOT NULL
                     ORDER BY updated_at DESC LIMIT 1",
                    params![path],
                    |row| row.get(0),
                )
                .ok();

            let mut by_adapter = std::collections::BTreeMap::new();
            {
                let mut s2 = conn.prepare(
                    "SELECT source, COUNT(*) FROM sessions
                     WHERE project_path = ?1 GROUP BY source",
                )?;
                let pairs: Vec<(String, i64)> = s2
                    .query_map(params![path], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for (src, cnt) in pairs {
                    by_adapter.insert(src, cnt);
                }
            }

            results.push(ProjectSummary {
                project_path: path,
                name,
                session_count,
                message_count,
                last_title,
                last_updated,
                by_adapter,
            });
        }
        Ok(results)
    }

    pub fn daily_session_counts(&self, days: u32) -> Result<Vec<TimelineEntry>> {
        let conn = self.conn.lock().unwrap();
        let offset = format!("-{days} days");
        let mut stmt = conn.prepare(
            "SELECT DATE(updated_at) as day, source, COUNT(*) as cnt,
                    COALESCE(SUM(message_count), 0) as msgs
             FROM sessions
             WHERE updated_at >= DATE('now', ?1)
             GROUP BY day, source
             ORDER BY day ASC",
        )?;
        let rows = stmt
            .query_map(params![offset], |row| {
                Ok(TimelineEntry {
                    date: row.get(0)?,
                    adapter: row.get(1)?,
                    sessions: row.get(2)?,
                    messages: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
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
