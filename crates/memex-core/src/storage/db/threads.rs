//! L5「主题线索（Threads）」的存储层。
//!
//! 设计要点：
//! - `threads` + `thread_sessions` 是一对 N:N 表。一个 session 可以同时
//!   属于多个 thread，例如「memex 桌面化」+「Tauri 多窗口」。
//! - 写入路径只走 `upsert_thread_with_sessions`：原子地拿 / 新建 thread 记
//!   录、清空它已有的 link、重新批量插入。重生成时不会留下孤儿 link。
//! - 删除 thread 时 `thread_sessions` 走 ON DELETE CASCADE，所以
//!   `delete_thread(name)` 一句即可。
//!
//! 频率说明：thread 生成在 ingest 末尾按"每周一次或手动触发"调用一次，
//! 不在高频路径，所以这里查询走最朴素的 SQL，不做 prepared cache。

use anyhow::Result;
use chrono::Utc;
use rusqlite::params;
use serde::Serialize;

use super::{Db, sessions::SessionRow};

/// `threads` 表的一行 + 用于 list_threads 的展示信息。
/// 只需 Serialize（IPC 出去给前端），Deserialize 不需要——
/// 来自 LLM 的 thread 草稿走 ThreadDraft，不复用此类型。
///
/// 卡片视图额外需要的派生字段（`first_session_at` / `last_session_at` /
/// `projects` / `adapters`）通过 list_threads SQL 一次 JOIN 聚合返回，
/// 避免前端 N+1。
#[derive(Debug, Clone, Serialize)]
pub struct ThreadRow {
    pub id: i64,
    pub name: String,
    pub summary: String,
    pub session_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// 命中会话中最早的 created_at（按所有 sessions 时间跨度）。可能为空（没有命中或全部 session 已删）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_session_at: Option<String>,
    /// 命中会话中最晚的 updated_at。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_session_at: Option<String>,
    /// 涉及的项目（去重后用 `\n` 串联，前端 split），便于一次返回数组。
    /// 用 `\n` 而不是 `,` 因为项目路径里有逗号的可能比换行多。
    #[serde(default)]
    pub projects: Vec<String>,
    /// 涉及的适配器（去重后），如 cursor / claude_code。
    #[serde(default)]
    pub adapters: Vec<String>,
}

/// 一个 thread 的详情：基础信息 + 命中的 session 列表（取 SessionRow 给前端复用）。
#[derive(Debug, Clone, Serialize)]
pub struct ThreadDetail {
    pub thread: ThreadRow,
    pub sessions: Vec<SessionRow>,
}

/// LLM 聚类输出的一个 thread 草稿，准备落库。
#[derive(Debug, Clone)]
pub struct ThreadDraft {
    pub name: String,
    pub summary: String,
    pub session_ids: Vec<String>,
}

impl Db {
    /// 原子地 upsert 一个 thread + 重建它的 session links。
    /// - 如果 `name` 已经存在，更新 `summary` / `session_count` / `updated_at`，
    ///   并把旧的 `thread_sessions` 全部删掉再重插，避免漂移。
    /// - `session_ids` 中的 session 必须真实存在（外键），不存在的会被静默跳过。
    pub fn upsert_thread_with_sessions(&self, draft: &ThreadDraft) -> Result<i64> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();

        // 1) upsert threads 主表
        let existing_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM threads WHERE name = ?1",
                params![draft.name],
                |row| row.get(0),
            )
            .ok();

        let thread_id = match existing_id {
            Some(id) => {
                tx.execute(
                    "UPDATE threads
                     SET summary = ?1, session_count = ?2, updated_at = ?3
                     WHERE id = ?4",
                    params![
                        draft.summary,
                        draft.session_ids.len() as i64,
                        now,
                        id
                    ],
                )?;
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO threads (name, summary, session_count, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![
                        draft.name,
                        draft.summary,
                        draft.session_ids.len() as i64,
                        now
                    ],
                )?;
                tx.last_insert_rowid()
            }
        };

        // 2) 清空旧 link
        tx.execute(
            "DELETE FROM thread_sessions WHERE thread_id = ?1",
            params![thread_id],
        )?;

        // 3) 重插 link。外键不匹配（session 不存在）的行 SQLite 会返回错误，
        //    我们这里允许部分失败 —— 已删掉的历史 session 不影响其它 link。
        for sid in &draft.session_ids {
            let r = tx.execute(
                "INSERT OR IGNORE INTO thread_sessions
                 (thread_id, session_id, confidence, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![thread_id, sid, 1.0_f64, now],
            );
            if let Err(e) = r {
                tracing::warn!(
                    "thread_session link skipped: thread={} session={} err={}",
                    thread_id,
                    sid,
                    e
                );
            }
        }

        // 4) 真实 session_count 取 link 数量（即便有部分跳过也准确）
        tx.execute(
            "UPDATE threads
             SET session_count = (SELECT COUNT(*) FROM thread_sessions WHERE thread_id = ?1)
             WHERE id = ?1",
            params![thread_id],
        )?;

        tx.commit()?;
        Ok(thread_id)
    }

    /// 列出全部 thread，按 updated_at DESC + 分页，并一次返回卡片视图需要的
    /// 派生字段（时间跨度、项目集合、适配器集合）。
    ///
    /// SQL 思路：
    /// - 主查询从 `threads` 出发，按 updated_at 排序分页（小表，快）。
    /// - 用相关子查询拿 min/max 时间，避免对 thread_sessions 做笛卡尔积聚合。
    /// - 项目 / 适配器走 GROUP_CONCAT 接 substr 折叠成 `\n` 分隔字符串，前端 split。
    ///   GROUP_CONCAT 默认逗号分隔，这里用 `CHAR(10)` 作分隔符避免与 project_path
    ///   中的逗号冲突；DISTINCT 去重在子查询内做。
    pub fn list_threads_paged(&self, limit: usize, offset: usize) -> Result<Vec<ThreadRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.name, t.summary, t.session_count, t.created_at, t.updated_at,
                    (SELECT MIN(s.created_at) FROM thread_sessions ts
                     JOIN sessions s ON s.id = ts.session_id
                     WHERE ts.thread_id = t.id) AS first_session_at,
                    (SELECT MAX(s.updated_at) FROM thread_sessions ts
                     JOIN sessions s ON s.id = ts.session_id
                     WHERE ts.thread_id = t.id) AS last_session_at,
                    (SELECT GROUP_CONCAT(p, CHAR(10)) FROM
                       (SELECT DISTINCT COALESCE(s.project_path, '') AS p
                        FROM thread_sessions ts
                        JOIN sessions s ON s.id = ts.session_id
                        WHERE ts.thread_id = t.id AND s.project_path IS NOT NULL AND s.project_path != ''
                        ORDER BY p)
                    ) AS projects_csv,
                    (SELECT GROUP_CONCAT(a, CHAR(10)) FROM
                       (SELECT DISTINCT s.source AS a
                        FROM thread_sessions ts
                        JOIN sessions s ON s.id = ts.session_id
                        WHERE ts.thread_id = t.id
                        ORDER BY a)
                    ) AS adapters_csv
             FROM threads t
             ORDER BY t.updated_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                let first_session_at: Option<String> = row.get(6).ok();
                let last_session_at: Option<String> = row.get(7).ok();
                let projects_csv: Option<String> = row.get(8).ok();
                let adapters_csv: Option<String> = row.get(9).ok();
                Ok(ThreadRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    summary: row.get(2)?,
                    session_count: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    first_session_at,
                    last_session_at,
                    projects: split_csv(projects_csv.as_deref()),
                    adapters: split_csv(adapters_csv.as_deref()),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 取一个 thread 的详情 + 它命中的 session 列表（按 sessions.updated_at DESC）。
    /// session 列表复用 SessionRow，前端就能直接复用 LibrarySessionListItem。
    pub fn get_thread_detail(&self, thread_id: i64) -> Result<Option<ThreadDetail>> {
        let conn = self.conn.lock().unwrap();

        // detail 不分页，所以即使 thread 命中 100 个 session，子查询代价也可接受。
        let thread = conn
            .query_row(
                "SELECT t.id, t.name, t.summary, t.session_count, t.created_at, t.updated_at,
                        (SELECT MIN(s.created_at) FROM thread_sessions ts
                         JOIN sessions s ON s.id = ts.session_id
                         WHERE ts.thread_id = t.id),
                        (SELECT MAX(s.updated_at) FROM thread_sessions ts
                         JOIN sessions s ON s.id = ts.session_id
                         WHERE ts.thread_id = t.id),
                        (SELECT GROUP_CONCAT(p, CHAR(10)) FROM
                           (SELECT DISTINCT COALESCE(s.project_path, '') AS p
                            FROM thread_sessions ts
                            JOIN sessions s ON s.id = ts.session_id
                            WHERE ts.thread_id = t.id AND s.project_path IS NOT NULL AND s.project_path != ''
                            ORDER BY p)
                        ),
                        (SELECT GROUP_CONCAT(a, CHAR(10)) FROM
                           (SELECT DISTINCT s.source AS a
                            FROM thread_sessions ts
                            JOIN sessions s ON s.id = ts.session_id
                            WHERE ts.thread_id = t.id
                            ORDER BY a)
                        )
                 FROM threads t WHERE t.id = ?1",
                params![thread_id],
                |row| {
                    let first_session_at: Option<String> = row.get(6).ok();
                    let last_session_at: Option<String> = row.get(7).ok();
                    let projects_csv: Option<String> = row.get(8).ok();
                    let adapters_csv: Option<String> = row.get(9).ok();
                    Ok(ThreadRow {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        summary: row.get(2)?,
                        session_count: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                        first_session_at,
                        last_session_at,
                        projects: split_csv(projects_csv.as_deref()),
                        adapters: split_csv(adapters_csv.as_deref()),
                    })
                },
            )
            .ok();

        let Some(thread) = thread else {
            return Ok(None);
        };

        // SELECT 列要和 list_sessions_paged 保持完全一致——前端 SessionRow
        // 解析复用同一份字段约定。
        let mut stmt = conn.prepare(
            "SELECT s.id, s.source, s.project_path, s.title, s.message_count,
                    s.created_at, s.updated_at,
                    sm.title AS summary_title,
                    (SELECT substr(m.content, 1, 120)
                     FROM messages m
                     WHERE m.session_id = s.id AND m.role = 'user'
                     ORDER BY m.source_offset ASC LIMIT 1) AS first_user_message,
                    s.intent
             FROM sessions s
             INNER JOIN thread_sessions ts ON ts.session_id = s.id
             LEFT JOIN summaries sm
                ON sm.session_id = s.id AND sm.level = 'L2_session'
             WHERE ts.thread_id = ?1
             ORDER BY s.updated_at DESC",
        )?;
        let sessions = stmt
            .query_map(params![thread_id], |row| {
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
                    intent: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(Some(ThreadDetail { thread, sessions }))
    }

    /// 删除一个 thread（含所有 link，靠 ON DELETE CASCADE）。
    pub fn delete_thread(&self, thread_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM threads WHERE id = ?1", params![thread_id])?;
        Ok(())
    }

    /// thread 总数（用于「N 条线索」展示）。
    pub fn count_threads(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM threads", [], |row| row.get(0))?;
        Ok(n)
    }
}

/// 把 `GROUP_CONCAT(col, CHAR(10))` 的结果按换行拆成数组，并过滤空字符串。
fn split_csv(s: Option<&str>) -> Vec<String> {
    match s {
        None => Vec::new(),
        Some(raw) => raw
            .split('\n')
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(|x| x.to_string())
            .collect(),
    }
}
