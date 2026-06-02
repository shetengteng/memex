//! `sources` 表 I/O —— 登记每一个被扫过的 adapter 文件，
//! 以及我们上一次消费到的 byte offset / mtime。

use anyhow::Result;
use rusqlite::params;

use super::Db;
use crate::storage::models::SourceState;

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
                state.last_scan.to_rfc3339()
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
}
