//! 不属于"按表 CRUD"模块的只读查询：
//!   * `doctor` 数据（schema 版本、FTS 健康、adapter 状态、各表计数）
//!   * `access_log` 写入辅助
//!   * 会话 ID 前缀查询（`memex session show <prefix>` 用）
//!
//! 按表 CRUD 在 `db/{sources,sessions,messages,chunks,kv}.rs` 各自的模块里。

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::db::Db;

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

#[derive(Debug, Clone, Serialize)]
pub struct TimelineEntry {
    pub date: String,
    pub adapter: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsBreakdown {
    pub by_adapter: std::collections::BTreeMap<String, i64>,
    pub by_project: std::collections::BTreeMap<String, i64>,
    pub recent_7d_sessions: i64,
    pub recent_7d_messages: i64,
    pub recent_30d_sessions: i64,
    pub recent_30d_messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub project_path: String,
    pub name: String,
    pub session_count: i64,
    pub message_count: i64,
    pub last_title: Option<String>,
    pub last_updated: String,
    pub by_adapter: std::collections::BTreeMap<String, i64>,
}

/// Workload 分析数据，对应 Dashboard 的 Workload tab。
/// 所有计数仅覆盖过去 `days` 天（前端选 7/30 等）。
#[derive(Debug, Clone, Serialize)]
pub struct WorkloadReport {
    /// 整个时间窗的天数
    pub days: u32,
    /// 每日 session/message 数（GitHub-style 日历视图原料）。
    /// 仅返回**有活动**的日子；前端按日期补齐空格子。
    pub daily: Vec<WorkloadDailyEntry>,
    /// 时间窗内每个 (weekday, hour) 桶的 session 数（168 个），
    /// 用于渲染 24h × 7-weekday 时段习惯叠加图。weekday=0 即周一。
    pub heatmap: Vec<WorkloadHeatmapCell>,
    /// 按 adapter 聚合的 session 数（饼图）
    pub by_adapter: Vec<WorkloadBucket>,
    /// 工作量最大的项目（条形图），top 10
    pub by_project: Vec<WorkloadProjectBucket>,
    /// 整体高总览：会话总数、消息总数、活跃天数、peak day 描述
    pub overall: WorkloadOverall,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadDailyEntry {
    /// 本地时间的 YYYY-MM-DD
    pub date: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadHeatmapCell {
    pub weekday: u8, // 0=Mon ... 6=Sun
    pub hour: u8,    // 0..24
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadBucket {
    pub key: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadProjectBucket {
    /// 完整 project_path，方便点击跳转
    pub project_path: String,
    /// path 的最后一段，UI 直接显示
    pub name: String,
    pub sessions: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkloadOverall {
    pub sessions: i64,
    pub messages: i64,
    pub active_days: i64,
    /// 这个时间窗里 sessions 最多的那一天（YYYY-MM-DD），可能为空
    pub peak_day: Option<String>,
    pub peak_day_sessions: i64,
}

impl Db {
    pub fn write_access_log(
        &self,
        query: &str,
        result_count: usize,
        latency_ms: u64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO access_log (query, result_count, latency_ms, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                query,
                result_count as i64,
                latency_ms as i64,
                chrono::Utc::now().to_rfc3339()
            ],
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
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok())
    }

    pub fn fts_health_check(&self) -> bool {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM chunks_fts", [], |row| {
            row.get::<_, i64>(0)
        })
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

    pub fn timeline(&self, days: u32) -> Result<Vec<TimelineEntry>> {
        let conn = self.conn.lock().unwrap();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339();
        // 按本地时间分桶，让用户看到的是自己时区的日期
        //（跨 UTC 0 点的会话特别需要这样处理）。
        let mut stmt = conn.prepare(
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

    pub fn stats_breakdown(&self) -> Result<StatsBreakdown> {
        let conn = self.conn.lock().unwrap();
        let mut by_adapter = std::collections::BTreeMap::new();
        {
            let mut stmt = conn.prepare(
                "SELECT source, COUNT(*) FROM sessions GROUP BY source",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in rows.flatten() {
                by_adapter.insert(r.0, r.1);
            }
        }
        let mut by_project = std::collections::BTreeMap::new();
        {
            let mut stmt = conn.prepare(
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

    pub fn list_project_summaries(&self) -> Result<Vec<ProjectSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
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

            let mut by_adapter = std::collections::BTreeMap::new();
            {
                let mut s2 = conn.prepare(
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

    pub fn daily_session_counts(&self, days: u32) -> Result<Vec<TimelineEntry>> {
        let conn = self.conn.lock().unwrap();
        let offset = format!("-{days} days");
        let mut stmt = conn.prepare(
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

    /// 生成 Workload 报告 — 一次性聚合给 Dashboard Workload tab。
    /// 注意：所有维度都基于 sessions 表的 updated_at（最后活动时间）和
    /// message_count 字段，避免触碰 messages 表（量大且 timestamp 可能为空）。
    pub fn workload_report(&self, days: u32) -> Result<WorkloadReport> {
        let conn = self.conn.lock().unwrap();
        // 语义：days=N 表示「包含今天在内、向前 N 天」的窗口。
        //   days=1  → 仅今天
        //   days=7  → 今天 + 前 6 天 = 一周
        //   days=30 → 今天 + 前 29 天 = 一月
        // SQLite 的 DATE('now', '-K days') 表示从今天向前数 K 天的那一天，
        // 所以窗口起点偏移量应为 N-1（而非 N，否则会多包含一天，
        // 导致「今天」桶把昨天的 session/消息也算进来）。
        let offset = format!("-{} days", days.saturating_sub(1));

        // 0) 每日明细 — 日历视图原料
        let mut daily_stmt = conn.prepare(
            "SELECT DATE(updated_at, 'localtime') as day,
                    COUNT(*) as sessions,
                    COALESCE(SUM(message_count), 0) as msgs
             FROM sessions
             WHERE updated_at >= DATE('now', 'localtime', ?1)
             GROUP BY day
             ORDER BY day ASC",
        )?;
        let daily: Vec<WorkloadDailyEntry> = daily_stmt
            .query_map(params![offset.clone()], |row| {
                Ok(WorkloadDailyEntry {
                    date: row.get(0)?,
                    sessions: row.get(1)?,
                    messages: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // 1) 热力图：168 桶（weekday × hour）。
        //    SQLite 的 strftime('%w') 返回 0=Sunday..6=Saturday，
        //    转成 ISO 风格 0=Mon..6=Sun 需要 (w + 6) % 7。
        //
        //    分桶口径有"两条数据通路"：
        //    A) 有 messages.timestamp 的 adapter（claude_code / codex / opencode）
        //       按真实 message 时间分桶 —— 同一个跨多小时的会话能正确摊开。
        //    B) 没有 messages.timestamp 的 adapter（cursor / continue）
        //       退化用 session.updated_at（last activity）分桶 —— 整个 session 落在
        //       它最后一次活动的那个小时桶里，这是数据源限制下能做到的最佳粗粒度。
        //    sessions 字段 = distinct session 数；messages 字段 = 该桶 message 数。
        //    两条通路按 (weekday, hour) 求和合并，最后整理成 168 桶数组。
        use std::collections::BTreeMap;
        let mut bucket: BTreeMap<(u8, u8), (i64, i64)> = BTreeMap::new();

        // A) 有 timestamp 的 messages：sessions = COUNT(DISTINCT session_id)，messages = COUNT(*)
        //    历史上这里有一个 `JOIN sessions s ON s.id = m.session_id`，但 heatmap 不用
        //    session 任何列，JOIN 等于额外做一次 covering-index lookup × messages 行数；
        //    在 30w+ messages 的真实库上把 30 天查询从 0.06s 拉到 6.3s（趋势页超时空白）。
        //    去掉 JOIN，依赖新的 idx_messages_timestamp 走 range scan。
        let mut msg_stmt = conn.prepare(
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
            .query_map(params![offset.clone()], |row| {
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
        let mut sess_stmt = conn.prepare(
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
            .query_map(params![offset.clone()], |row| {
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

        let heat_rows: Vec<WorkloadHeatmapCell> = bucket
            .into_iter()
            .map(|((weekday, hour), (sessions, messages))| WorkloadHeatmapCell {
                weekday,
                hour,
                sessions,
                messages,
            })
            .collect();

        // 2) 按 adapter 聚合
        let mut adp_stmt = conn.prepare(
            "SELECT source, COUNT(*) as sessions, COALESCE(SUM(message_count), 0) as msgs
             FROM sessions
             WHERE updated_at >= DATE('now', 'localtime', ?1)
             GROUP BY source
             ORDER BY sessions DESC",
        )?;
        let by_adapter: Vec<WorkloadBucket> = adp_stmt
            .query_map(params![offset.clone()], |row| {
                Ok(WorkloadBucket {
                    key: row.get(0)?,
                    sessions: row.get(1)?,
                    messages: row.get(2)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        // 3) 按 project 聚合 top 10。project_path NULL/空时归到 (no project) 桶里。
        let mut proj_stmt = conn.prepare(
            "SELECT COALESCE(NULLIF(project_path, ''), '(no project)') as project_path,
                    COUNT(*) as sessions,
                    COALESCE(SUM(message_count), 0) as msgs
             FROM sessions
             WHERE updated_at >= DATE('now', 'localtime', ?1)
             GROUP BY project_path
             ORDER BY sessions DESC
             LIMIT 10",
        )?;
        let by_project: Vec<WorkloadProjectBucket> = proj_stmt
            .query_map(params![offset.clone()], |row| {
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

        // 4) 整体汇总
        let (sessions, messages, active_days, peak_day, peak_day_sessions): (
            i64,
            i64,
            i64,
            Option<String>,
            i64,
        ) = {
            let total: (i64, i64) = conn
                .query_row(
                    "SELECT COUNT(*), COALESCE(SUM(message_count), 0) FROM sessions
                     WHERE updated_at >= DATE('now', 'localtime', ?1)",
                    params![offset.clone()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap_or((0, 0));

            let active_days: i64 = conn
                .query_row(
                    "SELECT COUNT(DISTINCT DATE(updated_at, 'localtime')) FROM sessions
                     WHERE updated_at >= DATE('now', 'localtime', ?1)",
                    params![offset.clone()],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            let peak: Option<(String, i64)> = conn
                .query_row(
                    "SELECT DATE(updated_at, 'localtime') as d, COUNT(*) as c
                     FROM sessions
                     WHERE updated_at >= DATE('now', 'localtime', ?1)
                     GROUP BY d
                     ORDER BY c DESC
                     LIMIT 1",
                    params![offset.clone()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();
            let (peak_day, peak_day_sessions) = match peak {
                Some((d, c)) => (Some(d), c),
                None => (None, 0),
            };
            (total.0, total.1, active_days, peak_day, peak_day_sessions)
        };

        Ok(WorkloadReport {
            days,
            daily,
            heatmap: heat_rows,
            by_adapter,
            by_project,
            overall: WorkloadOverall {
                sessions,
                messages,
                active_days,
                peak_day,
                peak_day_sessions,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_log() {
        let db = Db::open_in_memory().unwrap();
        db.write_access_log("redis", 5, 42).unwrap();
    }

    #[test]
    fn test_find_session_by_prefix() {
        let db = Db::open_in_memory().unwrap();
        db.insert_session("abc-12345", "claude_code", None, "/f.jsonl", 0, 0)
            .unwrap();
        assert_eq!(
            db.find_session_by_prefix("abc-1").unwrap().unwrap(),
            "abc-12345"
        );
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

    /// 直接插入 session 行（绕过 insert_message 的 updated_at 改写），
    /// 让 workload_report 在固定时间上做断言。
    fn ws_seed_session(
        db: &Db,
        id: &str,
        source: &str,
        project_path: Option<&str>,
        updated_at: &str,
        message_count: i64,
    ) {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO sessions (id, source, project_path, file_path, created_at, updated_at, message_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
            params![id, source, project_path, format!("/tmp/{id}.jsonl"), updated_at, message_count],
        )
        .unwrap();
    }

    #[test]
    fn test_workload_report_empty() {
        let db = Db::open_in_memory().unwrap();
        let r = db.workload_report(7).unwrap();
        assert_eq!(r.days, 7);
        assert_eq!(r.overall.sessions, 0);
        assert_eq!(r.overall.active_days, 0);
        assert!(r.daily.is_empty());
        assert!(r.by_adapter.is_empty());
        assert!(r.by_project.is_empty());
        assert!(r.heatmap.is_empty());
        assert!(r.overall.peak_day.is_none());
    }

    #[test]
    fn test_workload_report_aggregations() {
        let db = Db::open_in_memory().unwrap();
        // 用今天的本地时间，避免 cutoff (DATE('now','localtime','-N days')) 把数据剪掉。
        let now = chrono::Local::now();
        let today = now.format("%Y-%m-%d").to_string();
        let yesterday = (now - chrono::Duration::days(1))
            .format("%Y-%m-%d")
            .to_string();

        // 4 个 session：
        //   2 个 claude_code @ /a (今天)
        //   1 个 cursor      @ /a (昨天)
        //   1 个 cursor      @ /b (今天)
        ws_seed_session(
            &db,
            "s1",
            "claude_code",
            Some("/a"),
            &format!("{today}T10:00:00+08:00"),
            10,
        );
        ws_seed_session(
            &db,
            "s2",
            "claude_code",
            Some("/a"),
            &format!("{today}T15:00:00+08:00"),
            20,
        );
        ws_seed_session(
            &db,
            "s3",
            "cursor",
            Some("/a"),
            &format!("{yesterday}T11:00:00+08:00"),
            30,
        );
        ws_seed_session(
            &db,
            "s4",
            "cursor",
            Some("/b"),
            &format!("{today}T09:00:00+08:00"),
            5,
        );

        let r = db.workload_report(30).unwrap();
        assert_eq!(r.overall.sessions, 4);
        assert_eq!(r.overall.messages, 65);
        assert!(r.overall.active_days >= 1);

        let cc = r.by_adapter.iter().find(|b| b.key == "claude_code").unwrap();
        assert_eq!(cc.sessions, 2);
        assert_eq!(cc.messages, 30);

        let proj_a = r.by_project.iter().find(|p| p.project_path == "/a").unwrap();
        assert_eq!(proj_a.sessions, 3);
        assert_eq!(proj_a.name, "a");

        assert!(!r.heatmap.is_empty(), "heatmap should contain at least one cell");
        for cell in &r.heatmap {
            assert!(cell.weekday <= 6);
            assert!(cell.hour <= 23);
        }
        // 4 个 session 跨两天 → daily 应有 2 个桶
        assert_eq!(r.daily.len(), 2);
        let today_total: i64 = r.daily.iter().map(|d| d.sessions).sum();
        assert_eq!(today_total, 4);
    }

    #[test]
    fn test_workload_report_excludes_old_sessions() {
        let db = Db::open_in_memory().unwrap();
        // 一个 100 天前的 session 不应该出现在 30 天窗口里
        let old = (chrono::Local::now() - chrono::Duration::days(100))
            .format("%Y-%m-%dT%H:%M:%S+08:00")
            .to_string();
        ws_seed_session(&db, "old", "cursor", Some("/old"), &old, 1);
        let r = db.workload_report(30).unwrap();
        assert_eq!(r.overall.sessions, 0);
    }

    /// 验证 heatmap 的双通路口径：
    ///   有 messages.timestamp 的 session 走 message 维度分桶，
    ///   没 timestamp 的 session 退化用 session.updated_at。
    /// 关键断言：一个跨多小时的会话，message 通路里能摊到多个 hour 桶，
    /// 而 session 通路下永远只在 last_updated_at 的桶里出现一次。
    #[test]
    fn test_workload_heatmap_messages_vs_session_fallback() {
        let db = Db::open_in_memory().unwrap();
        // 同一天，三个 session：
        //   s1 claude_code 9 点和 13 点各 1 条 message（有 timestamp）
        //   s2 cursor     全无 timestamp，session.updated_at 15 点
        //   s3 codex      11 点 1 条 message（有 timestamp）
        let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
        ws_seed_session(
            &db,
            "s1",
            "claude_code",
            Some("/a"),
            &format!("{today_local}T13:30:00+00:00"),
            2,
        );
        ws_seed_session(
            &db,
            "s2",
            "cursor",
            Some("/a"),
            &format!("{today_local}T15:00:00+00:00"),
            7,
        );
        ws_seed_session(
            &db,
            "s3",
            "codex",
            Some("/b"),
            &format!("{today_local}T11:00:00+00:00"),
            1,
        );

        // 给 s1 / s3 插带 timestamp 的 message，s2 不插（模拟 cursor 没 timestamp）
        let conn = db.conn.lock().unwrap();
        for (mid, sid, ts) in [
            ("m1", "s1", format!("{today_local}T09:00:00+00:00")),
            ("m2", "s1", format!("{today_local}T13:00:00+00:00")),
            ("m3", "s3", format!("{today_local}T11:00:00+00:00")),
        ] {
            conn.execute(
                "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
                 VALUES (?1, ?2, 'user', 'x', ?3, 0, ?1)",
                params![mid, sid, ts],
            )
            .unwrap();
        }
        drop(conn);

        let r = db.workload_report(2).unwrap();

        // 把 heatmap 拍平到 hour，按 UTC 计算（测试在不同时区跑会浮动）。
        // 我们仅断言总和：3 条带 timestamp 的 message → message 通路出 3 桶（9/11/13），
        // 1 个无 timestamp 的 cursor session → fallback 通路出 1 桶（15）。
        let total_msgs: i64 = r.heatmap.iter().map(|c| c.messages).sum();
        // 来源：message 通路 3 条 + cursor session 通路用 message_count=7 → 共 10
        assert_eq!(total_msgs, 10, "messages aggregate across both paths");

        // sessions 聚合是按 (weekday, hour) 桶后再求和，跨多小时的 session 在每个桶里
        // 都贡献 1：
        //   s1 命中 9 / 13 两桶 = 2
        //   s3 命中 11 桶 = 1
        //   s2 fallback 命中 15 桶 = 1
        //   合计 = 4
        // 这是符合"按时间段看活动"语义的预期，而不是 distinct session 总数。
        let total_sessions: i64 = r.heatmap.iter().map(|c| c.sessions).sum();
        assert_eq!(total_sessions, 4);

        // 4 个不同的 hour 桶（9/11/13 + 15）。
        let distinct_hours: std::collections::HashSet<u8> =
            r.heatmap.iter().map(|c| c.hour).collect();
        assert_eq!(distinct_hours.len(), 4, "expected 4 distinct hour buckets, got {:?}", distinct_hours);
    }

    /// 回归测试：去掉 messages × sessions JOIN 之后，结果应当与有 JOIN 时一致。
    /// 旧 SQL 的 JOIN 只是用来过滤孤儿 message_id（实际项目里 messages.session_id
    /// 受外键约束，绝不出现孤儿），所以拆掉 JOIN 不会改变任何业务输出。
    #[test]
    fn test_workload_heatmap_no_orphan_messages_after_join_removal() {
        let db = Db::open_in_memory().unwrap();
        let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
        ws_seed_session(
            &db,
            "s1",
            "claude_code",
            Some("/a"),
            &format!("{today_local}T10:00:00+00:00"),
            3,
        );
        let conn = db.conn.lock().unwrap();
        for (mid, ts) in [
            ("m1", format!("{today_local}T08:00:00+00:00")),
            ("m2", format!("{today_local}T09:00:00+00:00")),
            ("m3", format!("{today_local}T10:00:00+00:00")),
        ] {
            conn.execute(
                "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
                 VALUES (?1, 's1', 'user', 'x', ?2, 0, ?1)",
                params![mid, ts],
            )
            .unwrap();
        }
        drop(conn);

        let r = db.workload_report(2).unwrap();
        let total_msgs: i64 = r.heatmap.iter().map(|c| c.messages).sum();
        assert_eq!(total_msgs, 3, "3 messages should all be counted");
        // 三个不同的小时桶 8 / 9 / 10
        let hours: std::collections::HashSet<u8> = r.heatmap.iter().map(|c| c.hour).collect();
        assert_eq!(hours.len(), 3);
    }

    /// 性能护栏：跑一个有 200 条带 timestamp 的 messages 的库，
    /// workload_report(30) 必须在 1 秒内完成。
    /// 真实场景里 30w+ messages 没索引会 6s+，这里用小数据集只验证 SQL 形态
    /// 不会再退化成 JOIN 全表扫描。
    #[test]
    fn test_workload_heatmap_completes_in_reasonable_time() {
        let db = Db::open_in_memory().unwrap();
        let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
        ws_seed_session(
            &db,
            "perf-s1",
            "claude_code",
            Some("/perf"),
            &format!("{today_local}T12:00:00+00:00"),
            200,
        );
        let conn = db.conn.lock().unwrap();
        for i in 0..200 {
            let hour = i % 24;
            let ts = format!("{today_local}T{:02}:00:00+00:00", hour);
            conn.execute(
                "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
                 VALUES (?1, 'perf-s1', 'user', 'x', ?2, ?3, ?1)",
                params![format!("perf-m{i}"), ts, i as i64],
            )
            .unwrap();
        }
        drop(conn);

        let start = std::time::Instant::now();
        let r = db.workload_report(30).unwrap();
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 1000,
            "workload_report(30) took {} ms, expected < 1000 ms",
            elapsed.as_millis()
        );
        let total_msgs: i64 = r.heatmap.iter().map(|c| c.messages).sum();
        assert_eq!(total_msgs, 200);
    }

    /// session 表里所有 message 都有 timestamp 时，session.updated_at fallback 不应
    /// 再被算一次（避免 double-count）。
    #[test]
    fn test_workload_heatmap_no_double_count_when_messages_have_ts() {
        let db = Db::open_in_memory().unwrap();
        let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
        ws_seed_session(
            &db,
            "s1",
            "claude_code",
            Some("/a"),
            &format!("{today_local}T13:00:00+00:00"),
            // 故意把 session.message_count 设成 99 —— 如果 fallback 误触发就会被加上
            99,
        );
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
             VALUES ('m1', 's1', 'user', 'x', ?1, 0, 'h1')",
            params![format!("{today_local}T13:00:00+00:00")],
        )
        .unwrap();
        drop(conn);

        let r = db.workload_report(2).unwrap();
        let total_msgs: i64 = r.heatmap.iter().map(|c| c.messages).sum();
        assert_eq!(total_msgs, 1, "must NOT include session.message_count=99 fallback");
        let total_sessions: i64 = r.heatmap.iter().map(|c| c.sessions).sum();
        assert_eq!(total_sessions, 1);
    }
}
