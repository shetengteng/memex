//! Read-side operations: FTS5 search with snippet, table counts, session list.

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::Db;
use crate::storage::models::SearchResult;

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub message_count: i64,
    pub updated_at: String,
}

impl Db {
    pub fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT c.id, c.session_id, c.message_id, c.chunk_type, c.content,
                    snippet(chunks_fts, 0, '<mark>', '</mark>', '...', 32) as snip,
                    rank,
                    s.source,
                    s.project_path,
                    m.timestamp
             FROM chunks_fts
             JOIN chunks c ON c.id = chunks_fts.rowid
             LEFT JOIN sessions s ON c.session_id = s.id
             LEFT JOIN messages m ON c.message_id = m.id
             WHERE chunks_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let results = stmt
            .query_map(params![query, limit as i64], |row| {
                Ok(SearchResult {
                    chunk_id: row.get(0)?,
                    session_id: row.get(1)?,
                    message_id: row.get(2)?,
                    chunk_type: row.get(3)?,
                    content: row.get(4)?,
                    snippet: row.get(5)?,
                    rank: row.get(6)?,
                    match_reason: String::new(),
                    adapter: row.get(7)?,
                    project: row.get(8)?,
                    timestamp: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(results)
    }

    pub fn session_count(&self) -> Result<u64> {
        self.scalar_count("SELECT COUNT(*) FROM sessions")
    }

    pub fn chunk_count(&self) -> Result<u64> {
        self.scalar_count("SELECT COUNT(*) FROM chunks")
    }

    pub fn message_count(&self) -> Result<u64> {
        self.scalar_count("SELECT COUNT(*) FROM messages")
    }

    fn scalar_count(&self, sql: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row(sql, [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source, project_path, message_count, updated_at
             FROM sessions
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    project_path: row.get(2)?,
                    message_count: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
