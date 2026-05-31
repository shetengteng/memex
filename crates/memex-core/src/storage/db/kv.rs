//! Generic key/value config bag (`kv`) and the redaction audit log
//! (`redactions`) — both write-ahead, no schema beyond what `schema.rs` sets.

use anyhow::Result;
use rusqlite::params;

use super::Db;

impl Db {
    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let val = conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| {
                row.get(0)
            })
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

    pub fn insert_redaction(
        &self,
        message_id: &str,
        session_id: &str,
        redaction_type: &str,
        original_length: usize,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO redactions (message_id, session_id, redaction_type, original_length, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                message_id,
                session_id,
                redaction_type,
                original_length as i64,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }
}
