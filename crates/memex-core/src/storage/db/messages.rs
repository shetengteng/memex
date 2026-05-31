//! Message inserts. Idempotent on `(content_hash, session_id)`; bumps
//! `sessions.message_count` and `sessions.updated_at` only on real inserts.

use anyhow::Result;
use rusqlite::params;

use super::Db;

impl Db {
    #[allow(clippy::too_many_arguments)]
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
}
