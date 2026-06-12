//! 通用 key/value 配置仓库（`kv`）与脱敏审计日志（`redactions`）——
//! 两张表都是 write-ahead，结构完全由 `schema.rs` 定义，这里不另起 schema。

use anyhow::Result;
use rusqlite::params;

use super::Db;

impl Db {
    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let val = conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| {
                row.get(0)
            })
            .ok();
        Ok(val)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
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
        let conn = self.conn.lock();
        let now = self.now_utc().to_rfc3339();
        conn.execute(
            "INSERT INTO redactions (message_id, session_id, redaction_type, original_length, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                message_id,
                session_id,
                redaction_type,
                original_length as i64,
                now
            ],
        )?;
        Ok(())
    }
}
