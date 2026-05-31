use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::db::Db;

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

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub db_exists: bool,
    pub schema_version: Option<u32>,
    pub session_count: u64,
    pub message_count: u64,
    pub chunk_count: u64,
    pub source_count: u64,
    pub fts_ok: bool,
    pub adapters: Vec<AdapterStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdapterStatus {
    pub name: String,
    pub file_count: u64,
    pub last_scan: Option<String>,
}

impl Db {
    pub fn get_session_detail(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let conn = self.conn.lock().unwrap();

        let session = conn
            .query_row(
                "SELECT id, source, project_path, file_path, title, created_at, updated_at, message_count
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

        let mut detail = match session {
            Some(d) => d,
            None => return Ok(None),
        };

        let mut stmt = conn.prepare(
            "SELECT id, role, content, timestamp FROM messages
             WHERE session_id = ?1
             ORDER BY source_offset ASC",
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
            .filter_map(|r| r.ok())
            .collect();

        Ok(Some(detail))
    }

    pub fn write_access_log(
        &self,
        query: &str,
        result_count: usize,
        latency_ms: u64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO access_log (query, result_count, latency_ms, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![query, result_count as i64, latency_ms as i64, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn source_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM sources", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn schema_version(&self) -> Result<Option<u32>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| row.get(0))
            .ok())
    }

    pub fn fts_health_check(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM chunks_fts", [], |row| row.get::<_, i64>(0))
            .is_ok()
    }

    pub fn adapter_statuses(&self) -> Result<Vec<AdapterStatus>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT adapter, COUNT(*) as cnt, MAX(last_scan) as ls
             FROM sources GROUP BY adapter",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(AdapterStatus {
                    name: row.get(0)?,
                    file_count: row.get(1)?,
                    last_scan: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |row| row.get(0))
            .ok())
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

    pub fn find_session_by_prefix(&self, prefix: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}%", prefix);
        Ok(conn
            .query_row(
                "SELECT id FROM sessions WHERE id LIKE ?1 ORDER BY updated_at DESC LIMIT 1",
                params![pattern],
                |row| row.get(0),
            )
            .ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_detail() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl").unwrap();
        let hash = blake3::hash(b"hello").to_hex().to_string();
        db.insert_message("m1", "s1", "user", "hello", None, 0, &hash).unwrap();

        let detail = db.get_session_detail("s1").unwrap().unwrap();
        assert_eq!(detail.id, "s1");
        assert_eq!(detail.messages.len(), 1);
        assert_eq!(detail.messages[0].role, "user");
    }

    #[test]
    fn test_session_detail_not_found() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.get_session_detail("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_access_log() {
        let db = Db::open_in_memory().unwrap();
        db.write_access_log("redis", 5, 42).unwrap();
    }

    #[test]
    fn test_kv_store() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.kv_get("missing").unwrap().is_none());
        db.kv_set("foo", "bar").unwrap();
        assert_eq!(db.kv_get("foo").unwrap().unwrap(), "bar");
        db.kv_set("foo", "baz").unwrap();
        assert_eq!(db.kv_get("foo").unwrap().unwrap(), "baz");
    }

    #[test]
    fn test_find_session_by_prefix() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session("abc-12345", "claude_code", None, "/f.jsonl").unwrap();
        assert_eq!(db.find_session_by_prefix("abc-1").unwrap().unwrap(), "abc-12345");
        assert!(db.find_session_by_prefix("zzz").unwrap().is_none());
    }

    #[test]
    fn test_fts_health() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.fts_health_check());
    }

    #[test]
    fn test_doctor_queries() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.schema_version().unwrap().is_some());
        assert_eq!(db.source_count().unwrap(), 0);
        assert!(db.adapter_statuses().unwrap().is_empty());
    }
}
