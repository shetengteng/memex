//! 会话摘要 CRUD —— 支持 L1（chunk）、L2（session）、L3（项目）、
//! L4（周期）四个层级。按 `(session_id, level)` 这对唯一键做 upsert。

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::Db;

#[derive(Debug, Clone, Serialize)]
pub struct SummaryRow {
    pub id: i64,
    pub session_id: String,
    pub level: String,
    pub title: Option<String>,
    pub summary: String,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    pub created_at: String,
}

/// 写入会话摘要的 payload。把 7 个零散参数收敛成一个 struct 是为了
/// 满足 `clippy::too_many_arguments`（规约 §6.2 同建议）：调用方使用
/// 字段名构造，比按位置传 7 个参数更不容易写错顺序。
pub struct SummaryUpsert<'a> {
    pub session_id: &'a str,
    /// `L1_chunk` / `L2_session` / `L3_project` / `L4_period`
    pub level: &'a str,
    pub title: Option<&'a str>,
    pub summary: &'a str,
    pub topics: &'a [String],
    pub decisions: &'a [String],
    /// 写入时刻 `sessions.message_count` 的快照。仅 `L2_session` 用到（方案 A
    /// 过期检测）；其他层级填 0 即可。
    pub message_count_at_creation: i64,
}

/// 写入"跨 session 聚合摘要"（项目 / 周 / 月 / 反思）的 payload。同
/// `SummaryUpsert`，把 7 个零散参数收敛成 struct。
pub struct AggregateSummaryUpsert<'a> {
    /// `project` / `weekly` / `monthly` / `daily` / `reflect`
    pub scope_type: &'a str,
    /// 配合 `scope_type` 的唯一 key（如 `project=/path`、`weekly=2026-W23`）。
    pub scope_key: &'a str,
    pub title: Option<&'a str>,
    pub summary: &'a str,
    pub topics: &'a [String],
    pub decisions: &'a [String],
    pub session_count: i64,
}

impl Db {
    /// 写入 / 更新一条会话摘要。
    ///
    /// `message_count_at_creation` 用于「过期检测」（方案 A）—— 写入时把当时
    /// `sessions.message_count` 一同存进来；下次 ingest 时若 `sessions.message_count`
    /// 已经超过此值，说明摘要生成后又有新消息写入，需要重新生成摘要。
    ///
    /// 非 L2_session 层级（L1/L3/L4）由其他写入路径管理，本字段记 0 即可。
    pub fn upsert_summary(&self, opts: SummaryUpsert<'_>) -> Result<()> {
        let SummaryUpsert {
            session_id,
            level,
            title,
            summary,
            topics,
            decisions,
            message_count_at_creation,
        } = opts;
        let conn = self.conn.lock();
        let topics_json = serde_json::to_string(topics)?;
        let decisions_json = serde_json::to_string(decisions)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO summaries (session_id, level, title, summary, topics_json, decisions_json, created_at, message_count_at_creation)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(session_id, level) DO UPDATE SET
                title = excluded.title,
                summary = excluded.summary,
                topics_json = excluded.topics_json,
                decisions_json = excluded.decisions_json,
                created_at = excluded.created_at,
                message_count_at_creation = excluded.message_count_at_creation",
            params![session_id, level, title, summary, topics_json, decisions_json, now, message_count_at_creation],
        )?;
        if level == "L2_session"
            && let Some(t) = title
        {
            conn.execute(
                "UPDATE sessions SET title = ?1 WHERE id = ?2 AND (title IS NULL OR title = '')",
                params![t, session_id],
            )?;
        }
        Ok(())
    }

    pub fn get_summary(&self, session_id: &str, level: &str) -> Result<Option<SummaryRow>> {
        let conn = self.conn.lock();
        let row = conn.query_row(
            "SELECT id, session_id, level, title, summary, topics_json, decisions_json, created_at
             FROM summaries WHERE session_id = ?1 AND level = ?2",
            params![session_id, level],
            |row| {
                let topics_json: String = row.get(5)?;
                let decisions_json: String = row.get(6)?;
                Ok(SummaryRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    level: row.get(2)?,
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    topics: serde_json::from_str(&topics_json).unwrap_or_default(),
                    decisions: serde_json::from_str(&decisions_json).unwrap_or_default(),
                    created_at: row.get(7)?,
                })
            },
        ).ok();
        Ok(row)
    }

    pub fn list_summaries(&self, session_id: &str) -> Result<Vec<SummaryRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, session_id, level, title, summary, topics_json, decisions_json, created_at
             FROM summaries WHERE session_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                let topics_json: String = row.get(5)?;
                let decisions_json: String = row.get(6)?;
                Ok(SummaryRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    level: row.get(2)?,
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    topics: serde_json::from_str(&topics_json).unwrap_or_default(),
                    decisions: serde_json::from_str(&decisions_json).unwrap_or_default(),
                    created_at: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_summary(&self, session_id: &str, level: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM summaries WHERE session_id = ?1 AND level = ?2",
            params![session_id, level],
        )?;
        if level == "L2_session" {
            conn.execute(
                "UPDATE sessions SET title = NULL WHERE id = ?1",
                params![session_id],
            )?;
        }
        Ok(deleted > 0)
    }

    /// 列出所有「需要生成 / 重新生成 L2 摘要」的 session id。
    ///
    /// 触发条件（任一满足即列入）：
    ///   1. 该 session 还没有 L2 摘要（`sm.id IS NULL`） — 老逻辑；
    ///   2. session.message_count > 摘要生成时的 message_count_at_creation
    ///      — 方案 A「过期检测」：摘要生成后又有新消息写入；
    ///
    /// 并附加冷却条件（方案 B）：仅当 `sessions.updated_at` 距离现在
    /// ≥ `cool_down_secs` 时才纳入候选。这样能避免「2 秒去抖刚触发摘要、
    /// 5 秒后又来新消息又触发摘要」的高频抖动 —— 对于 Claude Code 这种
    /// 「用完就关」的会话，冷却期一过摘要就一次生成完整内容。
    ///
    /// 如果 `cool_down_secs == 0`，则跳过冷却检查（兼容旧行为 / 测试）。
    pub fn sessions_needing_summary(
        &self,
        limit: usize,
        cool_down_secs: u64,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        // SQLite 接受 ISO 8601 字符串比较（按字典序等价于时间序）。
        // 用 cutoff = now - cool_down_secs，selector 选 updated_at <= cutoff 的会话。
        let cutoff = if cool_down_secs == 0 {
            // 远未来 cutoff 让条件恒成立 —— 即不施加冷却约束。
            "9999-12-31T23:59:59Z".to_string()
        } else {
            let cd = chrono::Duration::seconds(cool_down_secs as i64);
            (chrono::Utc::now() - cd).to_rfc3339()
        };

        let mut stmt = conn.prepare_cached(
            "SELECT s.id FROM sessions s
             LEFT JOIN summaries sm
               ON s.id = sm.session_id AND sm.level = 'L2_session'
             WHERE s.message_count >= 2
               AND s.updated_at <= ?1
               AND (
                 sm.id IS NULL
                 OR s.message_count > sm.message_count_at_creation
               )
             ORDER BY s.updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![cutoff, limit as i64], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 旧名：兼容老调用方（测试 / CLI）。委托给 `sessions_needing_summary`，
    /// `cool_down_secs = 0`（不施加冷却约束，保持原语义）。
    pub fn sessions_without_summary(&self, limit: usize) -> Result<Vec<String>> {
        self.sessions_needing_summary(limit, 0)
    }

    pub fn summary_count(&self) -> Result<u64> {
        let conn = self.conn.lock();
        Ok(conn.query_row("SELECT COUNT(*) FROM summaries", [], |row| row.get(0))?)
    }

    /// 有资格生成 L2 摘要的会话数。
    /// 阈值跟 `summarize_session_by_id` 里的 `messages.len() >= 2` 严格一致 —
    /// 只有 0 / 1 条消息的会话客观上拿不到摘要，不应计入「待生成」进度的分母，
    /// 否则会卡在永远凑不齐 100% 的尴尬数字（例如 919 个会话里有 19 个只有 1 条 → 上限 97.93%）。
    pub fn sessions_eligible_for_summary_count(&self) -> Result<u64> {
        let conn = self.conn.lock();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE message_count >= 2",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn chunks_with_summary_count(&self) -> Result<u64> {
        let conn = self.conn.lock();
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM chunks WHERE summary IS NOT NULL",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn upsert_aggregate_summary(&self, opts: AggregateSummaryUpsert<'_>) -> Result<()> {
        let AggregateSummaryUpsert {
            scope_type,
            scope_key,
            title,
            summary,
            topics,
            decisions,
            session_count,
        } = opts;
        let conn = self.conn.lock();
        let topics_json = serde_json::to_string(topics)?;
        let decisions_json = serde_json::to_string(decisions)?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO aggregate_summaries (scope_type, scope_key, title, summary, topics_json, decisions_json, session_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(scope_type, scope_key) DO UPDATE SET
                title = excluded.title,
                summary = excluded.summary,
                topics_json = excluded.topics_json,
                decisions_json = excluded.decisions_json,
                session_count = excluded.session_count,
                created_at = excluded.created_at",
            params![scope_type, scope_key, title, summary, topics_json, decisions_json, session_count, now],
        )?;
        Ok(())
    }

    pub fn list_aggregate_summaries(
        &self,
        scope_type: &str,
        limit: u32,
    ) -> Result<Vec<AggregateSummaryRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, scope_type, scope_key, title, summary, topics_json, decisions_json, session_count, created_at
             FROM aggregate_summaries
             WHERE scope_type = ?1
             ORDER BY scope_key DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![scope_type, limit], |row| {
                let topics_json: String = row.get(5)?;
                let decisions_json: String = row.get(6)?;
                Ok(AggregateSummaryRow {
                    id: row.get(0)?,
                    scope_type: row.get(1)?,
                    scope_key: row.get(2)?,
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    topics: serde_json::from_str(&topics_json).unwrap_or_default(),
                    decisions: serde_json::from_str(&decisions_json).unwrap_or_default(),
                    session_count: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_aggregate_summary(
        &self,
        scope_type: &str,
        scope_key: &str,
    ) -> Result<Option<AggregateSummaryRow>> {
        let conn = self.conn.lock();
        let row = conn.query_row(
            "SELECT id, scope_type, scope_key, title, summary, topics_json, decisions_json, session_count, created_at
             FROM aggregate_summaries WHERE scope_type = ?1 AND scope_key = ?2",
            params![scope_type, scope_key],
            |row| {
                let topics_json: String = row.get(5)?;
                let decisions_json: String = row.get(6)?;
                Ok(AggregateSummaryRow {
                    id: row.get(0)?,
                    scope_type: row.get(1)?,
                    scope_key: row.get(2)?,
                    title: row.get(3)?,
                    summary: row.get(4)?,
                    topics: serde_json::from_str(&topics_json).unwrap_or_default(),
                    decisions: serde_json::from_str(&decisions_json).unwrap_or_default(),
                    session_count: row.get(7)?,
                    created_at: row.get(8)?,
                })
            },
        ).ok();
        Ok(row)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AggregateSummaryRow {
    pub id: i64,
    pub scope_type: String,
    pub scope_key: String,
    pub title: Option<String>,
    pub summary: String,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    pub session_count: i64,
    pub created_at: String,
}
