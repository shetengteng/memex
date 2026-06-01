//! Session-level reads/writes plus the `SessionRow` / `SessionDetail` /
//! `MessageRow` shapes that the menubar IPC, MCP server, daemon HTTP API,
//! and `memex session show` CLI all depend on.

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::Db;

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub file_path: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
    pub messages: Vec<MessageRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: Option<String>,
}

impl Db {
    pub fn insert_session(
        &self,
        id: &str,
        source: &str,
        project_path: Option<&str>,
        file_path: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, source, project_path, file_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, source, project_path, file_path, now, now],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionRow>> {
        self.list_sessions_paged(limit, 0)
    }

    pub fn list_sessions_paged(&self, limit: usize, offset: usize) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source, project_path, title, message_count, created_at, updated_at
             FROM sessions ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    project_path: row.get(2)?,
                    title: row.get(3)?,
                    message_count: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_session_detail(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let conn = self.conn.lock().unwrap();
        let session = conn
            .query_row(
                "SELECT id, source, project_path, file_path, title,
                        created_at, updated_at, message_count
                 FROM sessions WHERE id = ?1",
                params![session_id],
                |row| {
                    Ok(SessionDetail {
                        id: row.get(0)?,
                        source: row.get(1)?,
                        project_path: row.get(2)?,
                        file_path: row.get(3)?,
                        title: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                        message_count: row.get(7)?,
                        messages: Vec::new(),
                    })
                },
            )
            .ok();

        let Some(mut detail) = session else {
            return Ok(None);
        };

        let mut stmt = conn.prepare(
            "SELECT id, role, content, timestamp FROM messages
             WHERE session_id = ?1 ORDER BY source_offset ASC",
        )?;
        detail.messages = stmt
            .query_map(params![session_id], |row| {
                Ok(MessageRow {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    timestamp: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(Some(detail))
    }

    pub fn session_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?)
    }

    pub fn message_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?)
    }

    pub fn list_sessions_by_project(&self, project_path: &str) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source, project_path, title, message_count, created_at, updated_at
             FROM sessions WHERE project_path = ?1
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map(params![project_path], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    project_path: row.get(2)?,
                    title: row.get(3)?,
                    message_count: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn distinct_projects(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT project_path FROM sessions
             WHERE project_path IS NOT NULL ORDER BY project_path",
        )?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_sessions_in_range(
        &self,
        after: &str,
        before: &str,
    ) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source, project_path, title, message_count, created_at, updated_at
             FROM sessions WHERE updated_at >= ?1 AND updated_at < ?2
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map(params![after, before], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    project_path: row.get(2)?,
                    title: row.get(3)?,
                    message_count: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

}
