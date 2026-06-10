//! 会话读取：分页列表、详情、按 project / 时间窗口的查询，以及全局计数。
//!
//! 子模块：
//!   * `filter` —— `list_sessions_filtered_paged` 的动态 SQL builder，单独
//!     拆开是因为它把 adapter/project/time/summary/query/sort 6 个维度的
//!     WHERE 拼接放在一起，跟"基础读取"语义不同。

mod filter;

use anyhow::Result;
use rusqlite::params;
use serde_rusqlite::from_rows;

use super::{MessageRow, SessionDetail, SessionListFilter, SessionRow};
use crate::storage::db::Db;

impl Db {
    pub fn list_sessions(&self, limit: usize) -> Result<Vec<SessionRow>> {
        self.list_sessions_paged(limit, 0)
    }

    pub fn list_sessions_paged(&self, limit: usize, offset: usize) -> Result<Vec<SessionRow>> {
        self.list_sessions_filtered_paged(&SessionListFilter::default(), limit, offset)
    }

    pub fn get_session_detail(&self, session_id: &str) -> Result<Option<SessionDetail>> {
        let conn = self.conn.lock();
        let session = conn
            .query_row(
                "SELECT id, source, project_path, file_path, title,
                        created_at, updated_at, message_count, intent
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
                        intent: row.get(8)?,
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
        if let Ok((title, summary, topics_json, decisions_json)) =
            conn.query_row::<(Option<String>, String, String, String), _, _>(
                "SELECT title, summary, topics_json, decisions_json
             FROM summaries WHERE session_id = ?1 AND level = ?2",
                params![session_id, "L2_session"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
        {
            detail.summary = Some(summary);
            detail.topics = serde_json::from_str(&topics_json).unwrap_or_default();
            detail.decisions = serde_json::from_str(&decisions_json).unwrap_or_default();
            if detail.title.is_none() {
                detail.title = title;
            }
        }

        // messages.timestamp 在 cursor / continue_dev 等 adapter 里整列为 NULL
        // （source 文件没记录），导致前端 session detail 的「每条消息时间」全部
        // 不显示。我们退化到 session.updated_at（整个会话的最后活动时间）让 UI
        // 至少能渲染一个时间戳，对一次性会话足够精确；对长会话用户看到的是
        // 会话级时间，比"无时间"友好。SerDe 仍走 Option<String>，COALESCE 决定
        // 取值，调用方拿到的永远是 Some。
        let mut stmt = conn.prepare_cached(
            "SELECT m.id, m.role, m.content,
                    COALESCE(m.timestamp, s.updated_at) AS ts
             FROM messages m
             JOIN sessions s ON s.id = m.session_id
             WHERE m.session_id = ?1
             ORDER BY m.source_offset ASC",
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
        let conn = self.conn.lock();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM sessions
             WHERE NOT (message_count = 0 AND created_at < datetime('now', '-1 day'))",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn message_count(&self) -> Result<u64> {
        let conn = self.conn.lock();
        Ok(conn.query_row(
            "SELECT COALESCE(SUM(message_count), 0) FROM sessions",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn list_sessions_by_project(&self, project_path: &str) -> Result<Vec<SessionRow>> {
        // 跟 list_sessions_paged 保持同一形态：JOIN L2 摘要 + 取第一条 user
        // 消息预览。context 注入用到这两个字段做"概览行"，否则只能拿 raw title。
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT s.id, s.source, s.project_path, s.title, s.message_count,
                    s.created_at, s.updated_at,
                    sm.title AS summary_title,
                    (SELECT substr(m.content, 1, 120)
                     FROM messages m
                     WHERE m.session_id = s.id AND m.role = 'user'
                     ORDER BY m.source_offset ASC LIMIT 1) AS first_user_message,
                    s.intent
             FROM sessions s
             LEFT JOIN summaries sm
                ON sm.session_id = s.id AND sm.level = 'L2_session'
             WHERE s.project_path = ?1
               AND NOT (s.message_count = 0
                        AND s.created_at < datetime('now', '-1 day'))
             ORDER BY s.updated_at DESC",
        )?;
        let rows = stmt.query(params![project_path])?;
        let out: Vec<SessionRow> =
            from_rows::<SessionRow>(rows).collect::<std::result::Result<_, _>>()?;
        Ok(out)
    }

    pub fn distinct_projects(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT DISTINCT project_path FROM sessions
             WHERE project_path IS NOT NULL ORDER BY project_path",
        )?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 取最近更新过的、带 `project_path` 的 session 的项目路径。
    ///
    /// 用于 `memex context` 的 IDE-cwd fallback：当 hook 启动时 `$PWD` 误指向
    /// `~/.cursor` / `~/.claude` 等 IDE 内部目录、`search_by_project` 三级匹配
    /// 全部失败时，用「用户最近活跃的项目」作为兜底 cwd，让 banner 还有内容
    /// 可注入。空数据库或全部 session 都缺 `project_path` 时返回 `None`。
    ///
    /// 过滤规则与 `list_sessions_*` 系列一致：跳过 message_count=0 且超 1 天
    /// 未补完的"孤儿/扫描中"会话，避免它们顶到最前面。
    pub fn latest_active_project(&self) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT project_path FROM sessions
             WHERE project_path IS NOT NULL AND project_path != ''
               AND NOT (message_count = 0 AND created_at < datetime('now', '-1 day'))
             ORDER BY updated_at DESC
             LIMIT 1",
        )?;
        let row: Result<String, rusqlite::Error> = stmt.query_row([], |row| row.get(0));
        match row {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 同 `distinct_projects`，但带每个 project_path 下的会话数。
    ///
    /// 用于 `context::matcher` 过滤掉单条孤儿会话（如 `/Users/me` 下偶然
    /// 写入的一条测试会话），避免它在 Tier 1 starts_with 阶段抢断真实
    /// 子项目命中。
    pub fn distinct_projects_with_counts(&self) -> Result<Vec<(String, i64)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT project_path, COUNT(*) AS n FROM sessions
             WHERE project_path IS NOT NULL
             GROUP BY project_path
             ORDER BY project_path",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_sessions_in_range(&self, after: &str, before: &str) -> Result<Vec<SessionRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, source, project_path, title, message_count, created_at, updated_at, intent
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
                    intent: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
