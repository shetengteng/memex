//! Write-side operations: source-state upsert, session/message/chunk inserts.
//! All methods take the global `Mutex<Connection>` lock; SQLite WAL mode means
//! readers don't block these writers but multiple writers serialise.

use anyhow::Result;
use rusqlite::params;

use super::Db;
use crate::storage::models::{Chunk, SourceState};

impl Db {
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
}
