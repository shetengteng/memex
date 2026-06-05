//! 会话级别（session）的读写，以及 menubar IPC、MCP server、daemon HTTP API
//! 和 `memex session show` CLI 共同依赖的 `SessionRow` / `SessionDetail` /
//! `MessageRow` 数据结构。

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::Db;

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// L2 摘要的标题（已经镜像写到了 `sessions.title`，但单独保留一份方便
    /// UI 区分"原始来源标题"和"LLM 生成的标题"）。当前与 `title` 同值，
    /// 预留供后续拆分使用。
    pub summary_title: Option<String>,
    /// 第一条 user 消息的预览（约 120 字），尚未生成摘要时作为 fallback，
    /// 避免 popup 列表里整条目为空。
    pub first_user_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub file_path: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
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
        self.insert_session_with_title(
            id,
            source,
            project_path,
            file_path,
            session_created_secs,
            session_mtime_secs,
            None,
        )
    }

    /// `title` 是 adapter 提供的"原始对话标题"（如 cursor composer.name、
    /// codex thread_name）。**仅在 sessions.title 当前为 NULL 时写入**：
    /// L2 摘要后续生成的更优 title 不会被这里覆盖。
    pub fn insert_session_with_title(
        &self,
        id: &str,
        source: &str,
        project_path: Option<&str>,
        file_path: &str,
        session_created_secs: u64,
        session_mtime_secs: u64,
        title: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
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
            (true, true) => "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    project_path = COALESCE(excluded.project_path, sessions.project_path),
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at",
            (true, false) => "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    project_path = COALESCE(excluded.project_path, sessions.project_path),
                    created_at = excluded.created_at",
            (false, true) => "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    project_path = COALESCE(excluded.project_path, sessions.project_path),
                    updated_at = excluded.updated_at",
            (false, false) => "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    project_path = COALESCE(excluded.project_path, sessions.project_path)",
        };
        conn.execute(
            sql,
            params![id, source, project_path, file_path, created_str, updated_str],
        )?;

        if let Some(t) = title.map(str::trim).filter(|s| !s.is_empty()) {
            conn.execute(
                "UPDATE sessions SET title = ?1 WHERE id = ?2 AND title IS NULL",
                params![t, id],
            )?;
        }
        Ok(())
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionRow>> {
        self.list_sessions_paged(limit, 0)
    }

    pub fn list_sessions_paged(&self, limit: usize, offset: usize) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.source, s.project_path, s.title, s.message_count,
                    s.created_at, s.updated_at,
                    sm.title AS summary_title,
                    (SELECT substr(m.content, 1, 120)
                     FROM messages m
                     WHERE m.session_id = s.id AND m.role = 'user'
                     ORDER BY m.source_offset ASC LIMIT 1) AS first_user_message
             FROM sessions s
             LEFT JOIN summaries sm
                ON sm.session_id = s.id AND sm.level = 'L2_session'
             WHERE NOT (s.message_count = 0
                        AND s.created_at < datetime('now', '-1 day'))
             ORDER BY s.updated_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    project_path: row.get(2)?,
                    title: row.get(3)?,
                    message_count: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    summary_title: row.get(7)?,
                    first_user_message: row.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_session_detail(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let conn = self.conn.lock().unwrap();
        let session = conn
            .query_row(
                "SELECT id, source, project_path, file_path, title,
                        created_at, updated_at, message_count
                 FROM sessions WHERE id = ?1",
                params![session_id],
                |row| {
                    Ok(SessionDetail {
                        id: row.get(0)?,
                        source: row.get(1)?,
                        project_path: row.get(2)?,
                        file_path: row.get(3)?,
                        title: row.get(4)?,
                        summary: None,
                        topics: Vec::new(),
                        decisions: Vec::new(),
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                        message_count: row.get(7)?,
                        messages: Vec::new(),
                    })
                },
            )
            .ok();

        let Some(mut detail) = session else {
            return Ok(None);
        };

        // 顺手把 L2 会话摘要一起取出来，UI 就能直接渲染 summary、topics、
        // decisions，不需要再绕一次 IPC。复用同一个已锁定的连接，
        // 避免再次抢锁。
        if let Ok((title, summary, topics_json, decisions_json)) = conn.query_row::<(Option<String>, String, String, String), _, _>(
            "SELECT title, summary, topics_json, decisions_json
             FROM summaries WHERE session_id = ?1 AND level = ?2",
            params![session_id, "L2_session"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ) {
            detail.summary = Some(summary);
            detail.topics = serde_json::from_str(&topics_json).unwrap_or_default();
            detail.decisions = serde_json::from_str(&decisions_json).unwrap_or_default();
            if detail.title.is_none() {
                detail.title = title;
            }
        }

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

    pub fn session_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM sessions
             WHERE NOT (message_count = 0 AND created_at < datetime('now', '-1 day'))",
            [], |row| row.get(0),
        )?)
    }

    pub fn message_count(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COALESCE(SUM(message_count), 0) FROM sessions", [], |row| row.get(0))?)
    }

    pub fn list_sessions_by_project(&self, project_path: &str) -> Result<Vec<SessionRow>> {
        // 跟 list_sessions_paged 保持同一形态：JOIN L2 摘要 + 取第一条 user
        // 消息预览。context 注入用到这两个字段做"概览行"，否则只能拿 raw title。
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT s.id, s.source, s.project_path, s.title, s.message_count,
                    s.created_at, s.updated_at,
                    sm.title AS summary_title,
                    (SELECT substr(m.content, 1, 120)
                     FROM messages m
                     WHERE m.session_id = s.id AND m.role = 'user'
                     ORDER BY m.source_offset ASC LIMIT 1) AS first_user_message
             FROM sessions s
             LEFT JOIN summaries sm
                ON sm.session_id = s.id AND sm.level = 'L2_session'
             WHERE s.project_path = ?1
               AND NOT (s.message_count = 0
                        AND s.created_at < datetime('now', '-1 day'))
             ORDER BY s.updated_at DESC",
        )?;
        let rows = stmt
            .query_map(params![project_path], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    project_path: row.get(2)?,
                    title: row.get(3)?,
                    message_count: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    summary_title: row.get(7)?,
                    first_user_message: row.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn distinct_projects(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT project_path FROM sessions
             WHERE project_path IS NOT NULL ORDER BY project_path",
        )?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 同 `distinct_projects`，但带每个 project_path 下的会话数。
    ///
    /// 用于 `context::matcher` 过滤掉单条孤儿会话（如 `/Users/me` 下偶然
    /// 写入的一条测试会话），避免它在 Tier 1 starts_with 阶段抢断真实
    /// 子项目命中。
    pub fn distinct_projects_with_counts(&self) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT project_path, COUNT(*) AS n FROM sessions
             WHERE project_path IS NOT NULL
             GROUP BY project_path
             ORDER BY project_path",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_sessions_in_range(
        &self,
        after: &str,
        before: &str,
    ) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source, project_path, title, message_count, created_at, updated_at
             FROM sessions WHERE updated_at >= ?1 AND updated_at < ?2
               AND NOT (message_count = 0 AND created_at < datetime('now', '-1 day'))
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map(params![after, before], |row| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    source: row.get(1)?,
                    project_path: row.get(2)?,
                    title: row.get(3)?,
                    message_count: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    summary_title: None,
                    first_user_message: None,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn update_session_project_path(
        &self,
        session_id: &str,
        project_path: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sessions SET project_path = ?1 WHERE id = ?2 AND (project_path IS NULL OR project_path = '')",
            params![project_path, session_id],
        )?;
        Ok(())
    }
}
