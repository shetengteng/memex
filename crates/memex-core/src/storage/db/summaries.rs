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
}
