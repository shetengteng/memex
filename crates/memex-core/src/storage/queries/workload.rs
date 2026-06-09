//! Workload report queries (Dashboard → Workload tab). One
//! `impl Db::workload_report` method that orchestrates four
//! independent aggregate queries and stitches the results into a
//! single [`WorkloadReport`]. Each piece lives in its own helper so
//! the SQL stays readable.

use std::collections::BTreeMap;

use anyhow::Result;
use rusqlite::{Connection, params};

use super::{
    WorkloadBucket, WorkloadDailyEntry, WorkloadHeatmapCell, WorkloadOverall,
    WorkloadProjectBucket, WorkloadReport,
};
use crate::storage::db::Db;

impl Db {
    /// 生成 Workload 报告 — 一次性聚合给 Dashboard Workload tab。
    ///
    /// Messages 字段口径：与 heatmap 同源的「两条数据通路」。
    ///   A) 有 `messages.timestamp` 的 adapter（claude_code/codex/opencode）
    ///      → 按 `messages.timestamp` 真实分桶；
    ///   B) 没 `messages.timestamp` 的 session（cursor/continue_dev）
    ///      → 退化按 `sessions.updated_at` + `sessions.message_count` 分桶。
    ///
    /// 历史 bug：`daily`/`overall.messages` 之前只用 (B) 通路 —— 把
    /// `SUM(sessions.message_count)` 算到 session.updated_at 的那一天，
    /// 导致跨天长会话每次活动就把整个生命周期的消息再"算"一遍今天。
    /// 现在两条通路 union，与 heatmap stat 同步。
    pub fn workload_report(&self, days: u32) -> Result<WorkloadReport> {
        let conn = self.conn.lock();
        // 语义：days=N 表示「包含今天在内、向前 N 天」的窗口。
        //   days=1  → 仅今天
        //   days=7  → 今天 + 前 6 天 = 一周
        //   days=30 → 今天 + 前 29 天 = 一月
        // SQLite 的 DATE('now', '-K days') 表示从今天向前数 K 天的那一天，
        // 所以窗口起点偏移量应为 N-1（而非 N，否则会多包含一天，
        // 导致「今天」桶把昨天的 session/消息也算进来）。
        let offset = format!("-{} days", days.saturating_sub(1));

        let daily = query_daily(&conn, &offset)?;
        let heatmap = query_heatmap(&conn, &offset)?;
        let by_adapter = query_by_adapter(&conn, &offset)?;
        let by_project = query_by_project(&conn, &offset)?;
        let overall = query_overall(&conn, &offset)?;

        Ok(WorkloadReport {
            days,
            daily,
            heatmap,
            by_adapter,
            by_project,
            overall,
        })
    }
}

/// (0) 每日明细 — 日历视图原料。两条通路 UNION 后 SUM。
fn query_daily(conn: &Connection, offset: &str) -> Result<Vec<WorkloadDailyEntry>> {
    let mut stmt = conn.prepare_cached(
        "WITH
            sessions_by_day AS (
                SELECT DATE(updated_at, 'localtime') AS day,
                       COUNT(*) AS sessions
                FROM sessions
                WHERE updated_at >= DATE('now', 'localtime', ?1)
                GROUP BY day
            ),
            msg_by_day_a AS (
                SELECT DATE(timestamp, 'localtime') AS day,
                       COUNT(*) AS msgs
                FROM messages
                WHERE timestamp IS NOT NULL
                  AND timestamp >= DATE('now', 'localtime', ?1)
                GROUP BY day
            ),
            msg_by_day_b AS (
                SELECT DATE(s.updated_at, 'localtime') AS day,
                       COALESCE(SUM(s.message_count), 0) AS msgs
                FROM sessions s
                WHERE s.updated_at >= DATE('now', 'localtime', ?1)
                  AND NOT EXISTS (
                      SELECT 1 FROM messages m
                      WHERE m.session_id = s.id AND m.timestamp IS NOT NULL
                  )
                GROUP BY day
            )
         SELECT sd.day,
                sd.sessions,
                COALESCE(a.msgs, 0) + COALESCE(b.msgs, 0) AS msgs
         FROM sessions_by_day sd
         LEFT JOIN msg_by_day_a a ON a.day = sd.day
         LEFT JOIN msg_by_day_b b ON b.day = sd.day
         ORDER BY sd.day ASC",
    )?;
    let rows = stmt
        .query_map(params![offset], |row| {
            Ok(WorkloadDailyEntry {
                date: row.get(0)?,
                sessions: row.get(1)?,
                messages: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// (1) 热力图 — 168 桶 (weekday × hour)。两条通路按 (w, h) 求和合并。
///
/// SQLite 的 strftime('%w') 返回 0=Sunday..6=Saturday，
/// 转成 ISO 风格 0=Mon..6=Sun 需要 `(w + 6) % 7`。
fn query_heatmap(conn: &Connection, offset: &str) -> Result<Vec<WorkloadHeatmapCell>> {
    let mut bucket: BTreeMap<(u8, u8), (i64, i64)> = BTreeMap::new();

    // A) 有 timestamp 的 messages：sessions = COUNT(DISTINCT session_id)，messages = COUNT(*)
    //    历史上这里有一个 `JOIN sessions s ON s.id = m.session_id`，但 heatmap 不用
    //    session 任何列，JOIN 等于额外做一次 covering-index lookup × messages 行数；
    //    在 30w+ messages 的真实库上把 30 天查询从 0.06s 拉到 6.3s（趋势页超时空白）。
    //    去掉 JOIN，依赖新的 idx_messages_timestamp 走 range scan。
    let mut msg_stmt = conn.prepare_cached(
        "SELECT
            (CAST(strftime('%w', m.timestamp, 'localtime') AS INTEGER) + 6) % 7 AS weekday,
            CAST(strftime('%H', m.timestamp, 'localtime') AS INTEGER) AS hour,
            COUNT(DISTINCT m.session_id) AS sessions,
            COUNT(*) AS msgs
         FROM messages m
         WHERE m.timestamp IS NOT NULL
           AND m.timestamp >= DATE('now', 'localtime', ?1)
         GROUP BY weekday, hour",
    )?;
    for row in msg_stmt
        .query_map(params![offset], |row| {
            let w: i64 = row.get(0)?;
            let h: i64 = row.get(1)?;
            let s: i64 = row.get(2)?;
            let m: i64 = row.get(3)?;
            Ok((w.clamp(0, 6) as u8, h.clamp(0, 23) as u8, s, m))
        })?
        .filter_map(|r| r.ok())
    {
        let (w, h, s, m) = row;
        let e = bucket.entry((w, h)).or_insert((0, 0));
        e.0 += s;
        e.1 += m;
    }

    // B) 退化路径：session 表里所有 message timestamp 都为 null 的 session，
    //    按 session.updated_at 分到一个桶里（message_count 走 session.message_count）。
    //    用 NOT EXISTS 取代 GROUP BY HAVING，让 SQLite 走 covering index 更快。
    let mut sess_stmt = conn.prepare_cached(
        "SELECT
            (CAST(strftime('%w', s.updated_at, 'localtime') AS INTEGER) + 6) % 7 AS weekday,
            CAST(strftime('%H', s.updated_at, 'localtime') AS INTEGER) AS hour,
            COUNT(*) AS sessions,
            COALESCE(SUM(s.message_count), 0) AS msgs
         FROM sessions s
         WHERE s.updated_at >= DATE('now', 'localtime', ?1)
           AND NOT EXISTS (
               SELECT 1 FROM messages m
               WHERE m.session_id = s.id AND m.timestamp IS NOT NULL
           )
         GROUP BY weekday, hour",
    )?;
    for row in sess_stmt
        .query_map(params![offset], |row| {
            let w: i64 = row.get(0)?;
            let h: i64 = row.get(1)?;
            let s: i64 = row.get(2)?;
            let m: i64 = row.get(3)?;
            Ok((w.clamp(0, 6) as u8, h.clamp(0, 23) as u8, s, m))
        })?
        .filter_map(|r| r.ok())
    {
        let (w, h, s, m) = row;
        let e = bucket.entry((w, h)).or_insert((0, 0));
        e.0 += s;
        e.1 += m;
    }

    Ok(bucket
        .into_iter()
        .map(
            |((weekday, hour), (sessions, messages))| WorkloadHeatmapCell {
                weekday,
                hour,
                sessions,
                messages,
            },
        )
        .collect())
}

/// (2) 按 adapter 聚合。
fn query_by_adapter(conn: &Connection, offset: &str) -> Result<Vec<WorkloadBucket>> {
    let mut stmt = conn.prepare_cached(
        "SELECT source, COUNT(*) as sessions, COALESCE(SUM(message_count), 0) as msgs
         FROM sessions
         WHERE updated_at >= DATE('now', 'localtime', ?1)
         GROUP BY source
         ORDER BY sessions DESC",
    )?;
    let rows = stmt
        .query_map(params![offset], |row| {
            Ok(WorkloadBucket {
                key: row.get(0)?,
                sessions: row.get(1)?,
                messages: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// (3) 按 project 聚合 top 10。
/// project_path NULL/空时归到 `(no project)` 桶里。
fn query_by_project(conn: &Connection, offset: &str) -> Result<Vec<WorkloadProjectBucket>> {
    let mut stmt = conn.prepare_cached(
        "SELECT COALESCE(NULLIF(project_path, ''), '(no project)') as project_path,
                COUNT(*) as sessions,
                COALESCE(SUM(message_count), 0) as msgs
         FROM sessions
         WHERE updated_at >= DATE('now', 'localtime', ?1)
         GROUP BY project_path
         ORDER BY sessions DESC
         LIMIT 10",
    )?;
    let rows = stmt
        .query_map(params![offset], |row| {
            let path: String = row.get(0)?;
            let name = path
                .rsplit('/')
                .find(|s| !s.is_empty())
                .unwrap_or(path.as_str())
                .to_string();
            Ok(WorkloadProjectBucket {
                project_path: path,
                name,
                sessions: row.get(1)?,
                messages: row.get(2)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// (4) 整体汇总：sessions / messages（A+B 两通路相加）/ active_days /
/// peak_day. 与 daily/heatmap 同源避免重复计入。
fn query_overall(conn: &Connection, offset: &str) -> Result<WorkloadOverall> {
    // 4 个标量 + 1 个 peak 行；用一个 SELECT 拿 4 个标量，peak 单独查。
    let (sessions, messages_a, messages_b, active_days): (i64, i64, i64, i64) = conn
        .query_row(
            "SELECT
                (SELECT COUNT(*) FROM sessions
                 WHERE updated_at >= DATE('now', 'localtime', ?1)),
                (SELECT COUNT(*) FROM messages
                 WHERE timestamp IS NOT NULL
                   AND timestamp >= DATE('now', 'localtime', ?1)),
                (SELECT COALESCE(SUM(s.message_count), 0) FROM sessions s
                 WHERE s.updated_at >= DATE('now', 'localtime', ?1)
                   AND NOT EXISTS (
                       SELECT 1 FROM messages m
                       WHERE m.session_id = s.id AND m.timestamp IS NOT NULL
                   )),
                (SELECT COUNT(DISTINCT DATE(updated_at, 'localtime'))
                 FROM sessions WHERE updated_at >= DATE('now', 'localtime', ?1))",
            params![offset],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap_or((0, 0, 0, 0));

    let (peak_day, peak_day_sessions) = conn
        .query_row(
            "SELECT DATE(updated_at, 'localtime') as d, COUNT(*) as c
             FROM sessions
             WHERE updated_at >= DATE('now', 'localtime', ?1)
             GROUP BY d ORDER BY c DESC LIMIT 1",
            params![offset],
            |row| Ok((Some(row.get(0)?), row.get(1)?)),
        )
        .unwrap_or((None, 0));

    Ok(WorkloadOverall {
        sessions,
        messages: messages_a + messages_b,
        active_days,
        peak_day,
        peak_day_sessions,
    })
}
