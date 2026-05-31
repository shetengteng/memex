use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use super::models::{Chunk, SearchResult, SourceState};
use serde::Serialize;

const SCHEMA_VERSION: u32 = 1;

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
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            );",
        )?;

        let version: Option<u32> = conn
            .query_row(
                "SELECT version FROM schema_version LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        if version.is_none() {
            conn.execute(
                "INSERT INTO schema_version (version) VALUES (?1)",
                params![SCHEMA_VERSION],
            )?;
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                adapter TEXT NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                last_offset INTEGER NOT NULL DEFAULT 0,
                last_mtime INTEGER NOT NULL DEFAULT 0,
                last_scan TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                project_path TEXT,
                file_path TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                message_count INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT,
                source_offset INTEGER NOT NULL DEFAULT 0,
                content_hash TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id TEXT NOT NULL REFERENCES messages(id),
                session_id TEXT NOT NULL REFERENCES sessions(id),
                chunk_type TEXT NOT NULL,
                content TEXT NOT NULL,
                redacted_content TEXT,
                position INTEGER NOT NULL DEFAULT 0,
                token_count INTEGER NOT NULL DEFAULT 0,
                metadata_json TEXT NOT NULL DEFAULT '{}'
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                content,
                content='chunks',
                content_rowid='id',
                tokenize='unicode61'
            );

            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO chunks_fts(rowid, content)
                VALUES (new.id, new.content);
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, content)
                VALUES ('delete', old.id, old.content);
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, content)
                VALUES ('delete', old.id, old.content);
                INSERT INTO chunks_fts(rowid, content)
                VALUES (new.id, new.content);
            END;

            CREATE TABLE IF NOT EXISTS access_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                result_count INTEGER NOT NULL DEFAULT 0,
                latency_ms INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;

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
            params![
                state.adapter,
                state.file_path,
                state.last_offset,
                state.last_mtime,
                state.last_scan.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_source_offset(&self, file_path: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let offset = conn
            .query_row(
                "SELECT last_offset FROM sources WHERE file_path = ?1",
                params![file_path],
                |row| row.get(0),
            )
            .unwrap_or(0u64);
        Ok(offset)
    }

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

    pub fn insert_message(
        &self,
        id: &str,
        session_id: &str,
        role: &str,
        content: &str,
        timestamp: Option<&str>,
        source_offset: u64,
        content_hash: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE content_hash = ?1 AND session_id = ?2)",
                params![content_hash, session_id],
                |row| row.get(0),
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
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn chunk_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn message_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))?;
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

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub message_count: i64,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{ChunkType, ChunkMetadata, Chunk};

    #[test]
    fn test_schema_init() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.session_count().unwrap(), 0);
        assert_eq!(db.chunk_count().unwrap(), 0);
    }

    #[test]
    fn test_insert_session_and_message() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session("s1", "claude_code", Some("/proj"), "/path/file.jsonl")
            .unwrap();
        assert_eq!(db.session_count().unwrap(), 1);

        let hash = blake3::hash(b"hello world").to_hex().to_string();
        let inserted = db
            .insert_message("m1", "s1", "user", "hello world", None, 0, &hash)
            .unwrap();
        assert!(inserted);
        assert_eq!(db.message_count().unwrap(), 1);

        // idempotent: same hash should not insert again
        let dup = db
            .insert_message("m2", "s1", "user", "hello world", None, 0, &hash)
            .unwrap();
        assert!(!dup);
        assert_eq!(db.message_count().unwrap(), 1);
    }

    #[test]
    fn test_insert_chunk_and_fts() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session("s1", "claude_code", None, "/f.jsonl").unwrap();
        let hash = blake3::hash(b"test content about redis pipeline").to_hex().to_string();
        db.insert_message("m1", "s1", "assistant", "test content about redis pipeline", None, 0, &hash).unwrap();

        let chunk = Chunk {
            id: None,
            message_id: "m1".to_string(),
            session_id: "s1".to_string(),
            chunk_type: ChunkType::Text,
            content: "test content about redis pipeline".to_string(),
            redacted_content: None,
            position: 0,
            token_count: 5,
            metadata: ChunkMetadata::default(),
        };
        let chunk_id = db.insert_chunk(&chunk).unwrap();
        assert!(chunk_id > 0);

        let results = db.fts_search("redis", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].snippet.contains("redis"));
        assert_eq!(results[0].adapter.as_deref(), Some("claude_code"));
    }

    #[test]
    fn test_source_offset() {
        let db = Db::open_in_memory().unwrap();
        let state = SourceState {
            adapter: "claude_code".to_string(),
            file_path: "/path/to/session.jsonl".to_string(),
            last_offset: 1024,
            last_mtime: 1717200000,
            last_scan: chrono::Utc::now(),
        };
        db.upsert_source(&state).unwrap();
        let offset = db.get_source_offset("/path/to/session.jsonl").unwrap();
        assert_eq!(offset, 1024);
    }
}
