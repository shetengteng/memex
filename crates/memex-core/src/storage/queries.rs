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

        // 1) 热力图：168 桶（weekday × hour），按 sessions 计数
        //    SQLite 的 strftime('%w') 返回 0=Sunday..6=Saturday，
        //    转成 ISO 风格 0=Mon..6=Sun 需要 (w + 6) % 7。
        let mut heat_stmt = conn.prepare(
            "SELECT
                (CAST(strftime('%w', updated_at, 'localtime') AS INTEGER) + 6) % 7 AS weekday,
                CAST(strftime('%H', updated_at, 'localtime') AS INTEGER) AS hour,
                COUNT(*) as sessions,
                COALESCE(SUM(message_count), 0) as msgs
             FROM sessions
             WHERE updated_at >= DATE('now', 'localtime', ?1)
             GROUP BY weekday, hour",
        )?;
        let heat_rows: Vec<WorkloadHeatmapCell> = heat_stmt
            .query_map(params![offset.clone()], |row| {
                let w: i64 = row.get(0)?;
                let h: i64 = row.get(1)?;
                Ok(WorkloadHeatmapCell {
                    weekday: w.clamp(0, 6) as u8,
                    hour: h.clamp(0, 23) as u8,
                    sessions: row.get(2)?,
                    messages: row.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
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
}
