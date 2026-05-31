mod schema;

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use super::models::{Chunk, SearchResult, SourceState};
use serde::Serialize;

pub struct Db {
    pub(crate) conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database: {}", path.display()))?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);",
        )?;

        let version: Option<u32> = conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
            .ok();

        if version.is_none() {
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                params![schema::SCHEMA_VERSION],
            )?;
        }

        conn.execute_batch(schema::SCHEMA_SQL)?;
        Ok(())
    }

    pub fn upsert_source(&self, state: &SourceState) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sources (adapter, file_path, last_offset, last_mtime, last_scan)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_path) DO UPDATE SET
                last_offset = excluded.last_offset,
                last_mtime = excluded.last_mtime,
                last_scan = excluded.last_scan",
            params![state.adapter, state.file_path, state.last_offset, state.last_mtime, state.last_scan.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn get_source_offset(&self, file_path: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let offset = conn
            .query_row("SELECT last_offset FROM sources WHERE file_path = ?1", params![file_path], |row| row.get(0))
            .unwrap_or(0u64);
        Ok(offset)
    }

    pub fn insert_session(&self, id: &str, source: &str, project_path: Option<&str>, file_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO sessions (id, source, project_path, file_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, source, project_path, file_path, now, now],
        )?;
        Ok(())
    }

    pub fn insert_message(
        &self, id: &str, session_id: &str, role: &str, content: &str,
        timestamp: Option<&str>, source_offset: u64, content_hash: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE content_hash = ?1 AND session_id = ?2)",
                params![content_hash, session_id], |row| row.get(0),
            )
            .unwrap_or(false);
        if exists {
            return Ok(false);
        }

        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, session_id, role, content, timestamp, source_offset, content_hash],
        )?;
        conn.execute(
            "UPDATE sessions SET message_count = message_count + 1, updated_at = ?1 WHERE id = ?2",
            params![chrono::Utc::now().to_rfc3339(), session_id],
        )?;
        Ok(true)
    }

    pub fn insert_chunk(&self, chunk: &Chunk) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let metadata_json = serde_json::to_string(&chunk.metadata)?;
        conn.execute(
            "INSERT INTO chunks (message_id, session_id, chunk_type, content, redacted_content, position, token_count, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                chunk.message_id, chunk.session_id, chunk.chunk_type.to_string(),
                chunk.content, chunk.redacted_content, chunk.position, chunk.token_count, metadata_json,
            ],
        )?;
        Ok(conn.last_insert_rowid())
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
                    chunk_id: row.get(0)?, session_id: row.get(1)?,
                    message_id: row.get(2)?, chunk_type: row.get(3)?,
                    content: row.get(4)?, snippet: row.get(5)?,
                    rank: row.get(6)?, match_reason: String::new(),
                    adapter: row.get(7)?, project: row.get(8)?, timestamp: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    pub fn session_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?)
    }

    pub fn chunk_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?)
    }

    pub fn message_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let conn = self.conn.lock().unwrap();
        let session = conn
            .query_row(
                "SELECT id, source, project_path, file_path, message_count, created_at, updated_at
                 FROM sessions WHERE id = ?1",
                params![session_id],
                |row| {
                    Ok(SessionDetail {
                        id: row.get(0)?,
                        source: row.get(1)?,
                        project_path: row.get(2)?,
                        file_path: row.get(3)?,
                        message_count: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                        messages: vec![],
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

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let val = conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| row.get(0))
            .ok();
        Ok(val)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source, project_path, message_count, updated_at
             FROM sessions ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(SessionRow {
                    id: row.get(0)?, source: row.get(1)?,
                    project_path: row.get(2)?, message_count: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub message_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub file_path: String,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
    pub messages: Vec<MessageRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{ChunkType, ChunkMetadata, Chunk};

    #[test]
    fn test_schema_init() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.session_count().unwrap(), 0);
    }

    #[test]
    fn test_insert_and_dedup() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl").unwrap();
        let hash = blake3::hash(b"hello").to_hex().to_string();
        assert!(db.insert_message("m1", "s1", "user", "hello", None, 0, &hash).unwrap());
        assert!(!db.insert_message("m2", "s1", "user", "hello", None, 0, &hash).unwrap());
        assert_eq!(db.message_count().unwrap(), 1);
    }

    #[test]
    fn test_fts_search() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session("s1", "claude_code", None, "/f.jsonl").unwrap();
        let hash = blake3::hash(b"redis pipeline").to_hex().to_string();
        db.insert_message("m1", "s1", "assistant", "redis pipeline", None, 0, &hash).unwrap();
        let chunk = Chunk {
            id: None, message_id: "m1".into(), session_id: "s1".into(),
            chunk_type: ChunkType::Text, content: "redis pipeline".into(),
            redacted_content: None, position: 0, token_count: 3,
            metadata: ChunkMetadata::default(),
        };
        db.insert_chunk(&chunk).unwrap();
        let results = db.fts_search("redis", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].adapter.as_deref(), Some("claude_code"));
    }

    #[test]
    fn test_source_offset() {
        let db = Db::open_in_memory().unwrap();
        let state = SourceState {
            adapter: "test".into(), file_path: "/test.jsonl".into(),
            last_offset: 1024, last_mtime: 0, last_scan: chrono::Utc::now(),
        };
        db.upsert_source(&state).unwrap();
        assert_eq!(db.get_source_offset("/test.jsonl").unwrap(), 1024);
    }
}
