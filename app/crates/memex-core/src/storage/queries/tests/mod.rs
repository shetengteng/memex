//! Unit tests for [`super`]. Split by topic so each file stays well
//! within the 300-line guideline; shared seed helper lives here.

mod general;
mod stats_basic;
mod workload_basic;
mod workload_daily;
mod workload_heatmap;

use rusqlite::params;

use crate::storage::db::Db;

/// 直接插入 session 行（绕过 insert_message 的 updated_at 改写），
/// 让 workload_report 在固定时间上做断言。
pub(super) fn ws_seed_session(
    db: &Db,
    id: &str,
    source: &str,
    project_path: Option<&str>,
    updated_at: &str,
    message_count: i64,
) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at, message_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
        params![id, source, project_path, format!("/tmp/{id}.jsonl"), updated_at, message_count],
    )
    .unwrap();
}
