//! 会话写入：新增（含 upsert）以及对 `project_path` / `intent` 的回填。

use anyhow::Result;
use rusqlite::params;

use super::NewSession;
use crate::storage::db::Db;

impl Db {
    pub fn insert_session(
        &self,
        id: &str,
        source: &str,
        project_path: Option<&str>,
        file_path: &str,
        session_created_secs: u64,
        session_mtime_secs: u64,
    ) -> Result<()> {
        self.insert_session_with_title(NewSession {
            id,
            source,
            project_path,
            file_path,
            session_created_secs,
            session_mtime_secs,
            title: None,
        })
    }

    pub fn insert_session_with_title(&self, opts: NewSession<'_>) -> Result<()> {
        let NewSession {
            id,
            source,
            project_path,
            file_path,
            session_created_secs,
            session_mtime_secs,
            title,
        } = opts;
        let conn = self.conn.lock();
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        let created_str = if session_created_secs > 0 {
            chrono::DateTime::<chrono::Utc>::from_timestamp(session_created_secs as i64, 0)
                .unwrap_or(now)
                .to_rfc3339()
        } else {
            now_str.clone()
        };
        let updated_str = if session_mtime_secs > 0 {
            chrono::DateTime::<chrono::Utc>::from_timestamp(session_mtime_secs as i64, 0)
                .unwrap_or(now)
                .to_rfc3339()
        } else {
            created_str.clone()
        };
        let has_real_created = session_created_secs > 0;
        let has_real_mtime = session_mtime_secs > 0;

        // 当 adapter 提供了真实时间时，在后续 ingest 时一并修正这两个时间戳；
        // 否则保留现有值不动，避免每次扫描都把时间往前推。
        let sql = match (has_real_created, has_real_mtime) {
            (true, true) => {
                "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    project_path = COALESCE(excluded.project_path, sessions.project_path),
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at"
            }
            (true, false) => {
                "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    project_path = COALESCE(excluded.project_path, sessions.project_path),
                    created_at = excluded.created_at"
            }
            (false, true) => {
                "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    project_path = COALESCE(excluded.project_path, sessions.project_path),
                    updated_at = excluded.updated_at"
            }
            (false, false) => {
                "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    project_path = COALESCE(excluded.project_path, sessions.project_path)"
            }
        };
        conn.execute(
            sql,
            params![
                id,
                source,
                project_path,
                file_path,
                created_str,
                updated_str
            ],
        )?;

        if let Some(t) = title.map(str::trim).filter(|s| !s.is_empty()) {
            conn.execute(
                "UPDATE sessions SET title = ?1 WHERE id = ?2 AND title IS NULL",
                params![t, id],
            )?;
        }
        Ok(())
    }

    pub fn update_session_project_path(&self, session_id: &str, project_path: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sessions SET project_path = ?1 WHERE id = ?2 AND (project_path IS NULL OR project_path = '')",
            params![project_path, session_id],
        )?;
        Ok(())
    }

    /// 把 L2 摘要 LLM 推断出来的「用户真实意图」一句话写到 `sessions.intent`。
    /// 每次摘要重生成都覆盖这一列（即便从有值变成 None，也写入 None，
    /// 保证 UI 能反映最新摘要结果，不会出现"重新生成后旧 intent 留在那里"的尴尬）。
    pub fn update_session_intent(&self, session_id: &str, intent: Option<&str>) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sessions SET intent = ?1 WHERE id = ?2",
            params![intent, session_id],
        )?;
        Ok(())
    }
}
