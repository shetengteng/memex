//! Session summary CRUD — supports L1 (chunk), L2 (session), L3 (project),
//! L4 (periodic) levels. Upserts by `(session_id, level)` unique pair.

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::Db;

#[derive(Debug, Clone, Serialize)]
pub struct SummaryRow {
    pub id: i64,
    pub session_id: String,
    pub level: String,
    pub title: Option<String>,
    pub summary: String,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    pub created_at: String,
}

impl Db {
    pub fn upsert_summary(
        &self,
        session_id: &str,
        level: &str,
        title: Option<&str>,
        summary: &str,
        topics: &[String],
        decisions: &[String],
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let topics_json = serde_json::to_string(topics)?;
        let decisions_json = serde_json::to_string(decisions)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO summaries (session_id, level, title, summary, topics_json, decisions_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(session_id, level) DO UPDATE SET
                title = excluded.title,
                summary = excluded.summary,
                topics_json = excluded.topics_json,
                decisions_json = excluded.decisions_json,
                created_at = excluded.created_at",
            params![session_id, level, title, summary, topics_json, decisions_json, now],
        )?;
        if level == "L2_session" {
            if let Some(t) = title {
                conn.execute(
                    "UPDATE sessions SET title = ?1 WHERE id = ?2",
                    params![t, session_id],
                )?;
            }
        }
        Ok(())
    }

    pub fn get_summary(&self, session_id: &str, level: &str) -> Result<Option<SummaryRow>> {
        let conn = self.conn.lock().unwrap();
        let row = conn.query_row(
            "SELECT id, session_id, level, title, summary, topics_json, decisions_json, created_at
             FROM summaries WHERE session_id = ?1 AND level = ?2",
            params![session_id, level],
            |row| {
                let topics_json: String = row.get(5)?;
                let decisions_json: String = row.get(6)?;
                Ok(SummaryRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    level: row.get(2)?,
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    topics: serde_json::from_str(&topics_json).unwrap_or_default(),
                    decisions: serde_json::from_str(&decisions_json).unwrap_or_default(),
                    created_at: row.get(7)?,
                })
            },
        ).ok();
        Ok(row)
    }

    pub fn list_summaries(&self, session_id: &str) -> Result<Vec<SummaryRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, level, title, summary, topics_json, decisions_json, created_at
             FROM summaries WHERE session_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                let topics_json: String = row.get(5)?;
                let decisions_json: String = row.get(6)?;
                Ok(SummaryRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    level: row.get(2)?,
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    topics: serde_json::from_str(&topics_json).unwrap_or_default(),
                    decisions: serde_json::from_str(&decisions_json).unwrap_or_default(),
                    created_at: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_summary(&self, session_id: &str, level: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM summaries WHERE session_id = ?1 AND level = ?2",
            params![session_id, level],
        )?;
        if level == "L2_session" {
            conn.execute(
                "UPDATE sessions SET title = NULL WHERE id = ?1",
                params![session_id],
            )?;
        }
        Ok(deleted > 0)
    }

    pub fn sessions_without_summary(&self, limit: usize) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT s.id FROM sessions s
             LEFT JOIN summaries sm ON s.id = sm.session_id AND sm.level = 'L2_session'
             WHERE sm.id IS NULL AND s.message_count >= 2
             ORDER BY s.updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn summary_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM summaries", [], |row| row.get(0))?)
    }

    pub fn chunks_with_summary_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM chunks WHERE summary IS NOT NULL", [], |row| row.get(0))?)
    }

    pub fn upsert_aggregate_summary(
        &self,
        scope_type: &str,
        scope_key: &str,
        title: Option<&str>,
        summary: &str,
        topics: &[String],
        decisions: &[String],
        session_count: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let topics_json = serde_json::to_string(topics)?;
        let decisions_json = serde_json::to_string(decisions)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO aggregate_summaries (scope_type, scope_key, title, summary, topics_json, decisions_json, session_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(scope_type, scope_key) DO UPDATE SET
                title = excluded.title,
                summary = excluded.summary,
                topics_json = excluded.topics_json,
                decisions_json = excluded.decisions_json,
                session_count = excluded.session_count,
                created_at = excluded.created_at",
            params![scope_type, scope_key, title, summary, topics_json, decisions_json, session_count, now],
        )?;
        Ok(())
    }

    pub fn list_aggregate_summaries(
        &self,
        scope_type: &str,
        limit: u32,
    ) -> Result<Vec<AggregateSummaryRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, scope_type, scope_key, title, summary, topics_json, decisions_json, session_count, created_at
             FROM aggregate_summaries
             WHERE scope_type = ?1
             ORDER BY scope_key DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![scope_type, limit], |row| {
                let topics_json: String = row.get(5)?;
                let decisions_json: String = row.get(6)?;
                Ok(AggregateSummaryRow {
                    id: row.get(0)?,
                    scope_type: row.get(1)?,
                    scope_key: row.get(2)?,
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    topics: serde_json::from_str(&topics_json).unwrap_or_default(),
                    decisions: serde_json::from_str(&decisions_json).unwrap_or_default(),
                    session_count: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_aggregate_summary(
        &self,
        scope_type: &str,
        scope_key: &str,
    ) -> Result<Option<AggregateSummaryRow>> {
        let conn = self.conn.lock().unwrap();
        let row = conn.query_row(
            "SELECT id, scope_type, scope_key, title, summary, topics_json, decisions_json, session_count, created_at
             FROM aggregate_summaries WHERE scope_type = ?1 AND scope_key = ?2",
            params![scope_type, scope_key],
            |row| {
                let topics_json: String = row.get(5)?;
                let decisions_json: String = row.get(6)?;
                Ok(AggregateSummaryRow {
                    id: row.get(0)?,
                    scope_type: row.get(1)?,
                    scope_key: row.get(2)?,
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    topics: serde_json::from_str(&topics_json).unwrap_or_default(),
                    decisions: serde_json::from_str(&decisions_json).unwrap_or_default(),
                    session_count: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        ).ok();
        Ok(row)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregateSummaryRow {
    pub id: i64,
    pub scope_type: String,
    pub scope_key: String,
    pub title: Option<String>,
    pub summary: String,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    pub session_count: i64,
    pub created_at: String,
}
