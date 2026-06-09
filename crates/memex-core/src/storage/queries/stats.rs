//! Stats / timeline / project-summary queries used by Dashboard's
//! Overview tab. Workload tab queries live in [`super::workload`].

use std::collections::BTreeMap;

use anyhow::Result;
use rusqlite::params;

use super::{ProjectSummary, StatsBreakdown, TimelineEntry};
use crate::storage::db::Db;

impl Db {
    /// Sessions / messages bucketed per local-time day × adapter for
    /// the last `days` days. Powers the "活动趋势" chart.
    pub fn timeline(&self, days: u32) -> Result<Vec<TimelineEntry>> {
        let conn = self.conn.lock();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();
        // 按本地时间分桶，让用户看到的是自己时区的日期
        //（跨 UTC 0 点的会话特别需要这样处理）。
        let mut stmt = conn.prepare_cached(
            "SELECT DATE(updated_at, 'localtime') as d, source, COUNT(*) as cnt,
                    SUM(message_count) as msgs
             FROM sessions WHERE updated_at >= ?1
             GROUP BY d, source ORDER BY d DESC",
        )?;
        let rows = stmt
            .query_map(params![cutoff], |row| {
                Ok(TimelineEntry {
                    date: row.get(0)?,
                    adapter: row.get(1)?,
                    sessions: row.get(2)?,
                    messages: row.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    /// Lifetime stats breakdown: by-adapter, top 20 by-project, plus
    /// rolling 7d/30d session+message totals.
    pub fn stats_breakdown(&self) -> Result<StatsBreakdown> {
        let conn = self.conn.lock();
        let mut by_adapter = BTreeMap::new();
        {
            let mut stmt =
                conn.prepare_cached("SELECT source, COUNT(*) FROM sessions GROUP BY source")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in rows.flatten() {
                by_adapter.insert(r.0, r.1);
            }
        }
        let mut by_project = BTreeMap::new();
        {
            let mut stmt = conn.prepare_cached(
                "SELECT project_path, COUNT(*) FROM sessions
                 WHERE project_path IS NOT NULL GROUP BY project_path
                 ORDER BY COUNT(*) DESC LIMIT 20",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in rows.flatten() {
                by_project.insert(r.0, r.1);
            }
        }
        let now = chrono::Utc::now();
        let d7 = (now - chrono::Duration::days(7)).to_rfc3339();
        let d30 = (now - chrono::Duration::days(30)).to_rfc3339();
        let recent_7d: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(message_count),0) FROM sessions WHERE updated_at >= ?1",
            params![d7],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or((0, 0));
        let recent_30d: (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(message_count),0) FROM sessions WHERE updated_at >= ?1",
            params![d30],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or((0, 0));
        Ok(StatsBreakdown {
            by_adapter,
            by_project,
            recent_7d_sessions: recent_7d.0,
            recent_7d_messages: recent_7d.1,
            recent_30d_sessions: recent_30d.0,
            recent_30d_messages: recent_30d.1,
        })
    }

    /// One row per (non-null) project_path: session_count, message
    /// total, last activity timestamp, last L2 title, and a
    /// per-adapter breakdown. Used by Library's project list.
    pub fn list_project_summaries(&self) -> Result<Vec<ProjectSummary>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT project_path,
                    COUNT(*) as cnt,
                    COALESCE(SUM(message_count), 0) as msgs,
                    MAX(updated_at) as last_upd
             FROM sessions
             WHERE project_path IS NOT NULL
             GROUP BY project_path
             ORDER BY last_upd DESC",
        )?;
        let base_rows: Vec<(String, i64, i64, String)> = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut results = Vec::with_capacity(base_rows.len());
        for (path, session_count, message_count, last_updated) in base_rows {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();

            let last_title: Option<String> = conn
                .query_row(
                    "SELECT title FROM sessions
                     WHERE project_path = ?1 AND title IS NOT NULL
                     ORDER BY updated_at DESC LIMIT 1",
                    params![path],
                    |row| row.get(0),
                )
                .ok();

            let mut by_adapter = BTreeMap::new();
            {
                let mut s2 = conn.prepare_cached(
                    "SELECT source, COUNT(*) FROM sessions
                     WHERE project_path = ?1 GROUP BY source",
                )?;
                let pairs: Vec<(String, i64)> = s2
                    .query_map(params![path], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for (src, cnt) in pairs {
                    by_adapter.insert(src, cnt);
                }
            }

            results.push(ProjectSummary {
                project_path: path,
                name,
                session_count,
                message_count,
                last_title,
                last_updated,
                by_adapter,
            });
        }
        Ok(results)
    }

    /// Per local-day × adapter session/message counts for the last
    /// `days` days. Differs from [`Db::timeline`] in that the SQL
    /// uses `DATE('now', '-N days')` instead of `updated_at >= cutoff`,
    /// keeping the window aligned to local midnight.
    pub fn daily_session_counts(&self, days: u32) -> Result<Vec<TimelineEntry>> {
        let conn = self.conn.lock();
        let offset = format!("-{days} days");
        let mut stmt = conn.prepare_cached(
            "SELECT DATE(updated_at, 'localtime') as day, source, COUNT(*) as cnt,
                    COALESCE(SUM(message_count), 0) as msgs
             FROM sessions
             WHERE updated_at >= DATE('now', 'localtime', ?1)
             GROUP BY day, source
             ORDER BY day ASC",
        )?;
        let rows = stmt
            .query_map(params![offset], |row| {
                Ok(TimelineEntry {
                    date: row.get(0)?,
                    adapter: row.get(1)?,
                    sessions: row.get(2)?,
                    messages: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
