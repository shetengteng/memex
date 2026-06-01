//! Chunk inserts and FTS5 search. Search is read-only and joins
//! `chunks_fts` ↔ `chunks` ↔ `sessions` ↔ `messages` to surface adapter,
//! project, and timestamp alongside the snippet.

use anyhow::Result;
use rusqlite::params;

use super::Db;
use crate::storage::models::{Chunk, SearchResult};

impl Db {
    pub fn insert_chunk(&self, chunk: &Chunk) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let metadata_json = serde_json::to_string(&chunk.metadata)?;
        conn.execute(
            "INSERT INTO chunks (message_id, session_id, chunk_type, content, redacted_content, position, token_count, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                chunk.message_id,
                chunk.session_id,
                chunk.chunk_type.to_string(),
                chunk.content,
                chunk.redacted_content,
                chunk.position,
                chunk.token_count,
                metadata_json,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn chunk_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?)
    }

    pub fn update_chunk_summary(&self, chunk_id: i64, summary: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE chunks SET summary = ?1 WHERE id = ?2",
            params![summary, chunk_id],
        )?;
        Ok(())
    }

    pub fn chunks_without_summary(&self, min_token_count: u32, limit: usize) -> Result<Vec<(i64, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content, redacted_content FROM chunks
             WHERE summary IS NULL AND token_count >= ?1
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![min_token_count, limit as i64], |row| {
                let id: i64 = row.get(0)?;
                let content: String = row.get(1)?;
                let redacted: Option<String> = row.get(2)?;
                Ok((id, redacted.unwrap_or(content), "".to_string()))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT c.id, c.session_id, c.message_id, c.chunk_type, c.content,
                    snippet(chunks_fts, 0, '<mark>', '</mark>', '...', 32) as snip, rank,
                    s.source, s.project_path, m.timestamp
             FROM chunks_fts
             JOIN chunks c ON c.id = chunks_fts.rowid
             LEFT JOIN sessions s ON c.session_id = s.id
             LEFT JOIN messages m ON c.message_id = m.id
             WHERE chunks_fts MATCH ?1
             ORDER BY rank LIMIT ?2",
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
}
