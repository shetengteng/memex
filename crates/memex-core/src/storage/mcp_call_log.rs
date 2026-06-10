//! `mcp_call_log` 表的写入与查询。
//!
//! 一次 MCP `tools/call` 写一行；前端 / CLI 用 [`Db::recent_mcp_calls`] 拉最近
//! N 条做"准实时事件流"，用 [`Db::mcp_call_stats_24h`] 拉 24 小时聚合（总数、
//! 平均延迟、按工具拆分、成功率）做活动卡顶部指标。
//!
//! 数据保留策略：不主动清理。占用按 50 字节 × 调用量估算，1 万次约 500KB，对
//! 个人本地 db 可忽略。需要清理时由用户走 `memex restore` 或将来加 admin 命令。

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::db::Db;

/// 单条调用记录的 IPC 形态。`tool_name` 走 snake_case 是因为 MCP 协议本身
/// 就是 snake_case（如 `get_project_context`），保留原样不做转换。
#[derive(Debug, Clone, Serialize)]
pub struct McpCallEntry {
    pub id: i64,
    pub occurred_at: String,
    pub tool_name: String,
    pub latency_ms: i64,
    pub success: bool,
    pub error_message: Option<String>,
}

/// 24 小时聚合视图。
#[derive(Debug, Clone, Serialize)]
pub struct McpCallStats24h {
    pub total: i64,
    pub success: i64,
    pub failed: i64,
    /// 仅对 success=1 的样本求平均，避免错误调用（通常很快返回）压低均值。
    pub avg_latency_ms: f64,
    pub by_tool: Vec<ToolBreakdown>,
    /// 24 小时内最近一次调用时间，给 UI 显示"上一次活动 N 分钟前"。
    pub last_call_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolBreakdown {
    pub tool_name: String,
    pub count: i64,
    pub avg_latency_ms: f64,
}

impl Db {
    /// 写一行调用记录。失败时返回 Err；调用方一般用 `let _ =` 吞掉错误，避免
    /// 单次写入失败影响 MCP 响应。
    pub fn insert_mcp_call(
        &self,
        tool_name: &str,
        latency_ms: u64,
        success: bool,
        error_message: Option<&str>,
    ) -> Result<()> {
        let occurred_at = self.now_utc().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO mcp_call_log (occurred_at, tool_name, latency_ms, success, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                occurred_at,
                tool_name,
                latency_ms as i64,
                if success { 1 } else { 0 },
                error_message,
            ],
        )?;
        Ok(())
    }

    /// 按 occurred_at 倒序取最近 N 条。limit 上限 500，超出截断（避免一次拉满
    /// 数据库给前端）。
    pub fn recent_mcp_calls(&self, limit: usize) -> Result<Vec<McpCallEntry>> {
        let capped = limit.min(500) as i64;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(
            "SELECT id, occurred_at, tool_name, latency_ms, success, error_message
             FROM mcp_call_log
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![capped], |row| {
                Ok(McpCallEntry {
                    id: row.get(0)?,
                    occurred_at: row.get(1)?,
                    tool_name: row.get(2)?,
                    latency_ms: row.get(3)?,
                    success: row.get::<_, i64>(4)? != 0,
                    error_message: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 24 小时滚动窗口聚合。SQL 三次扫描：total/success/failed/avg、by_tool、
    /// last_call_at。表行数预计 < 1 万量级，全表扫开销也很低。
    pub fn mcp_call_stats_24h(&self) -> Result<McpCallStats24h> {
        let cutoff = (self.now_utc() - chrono::Duration::hours(24)).to_rfc3339();
        let conn = self.conn.lock();

        let (total, success, failed, avg_latency_ms): (i64, i64, i64, Option<f64>) = conn
            .query_row(
                "SELECT
                     COUNT(*),
                     SUM(success),
                     SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END),
                     AVG(CASE WHEN success = 1 THEN latency_ms ELSE NULL END)
                 FROM mcp_call_log
                 WHERE occurred_at >= ?1",
                params![cutoff],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                        row.get::<_, Option<f64>>(3)?,
                    ))
                },
            )?;

        let mut stmt = conn.prepare(
            "SELECT tool_name, COUNT(*), AVG(latency_ms)
             FROM mcp_call_log
             WHERE occurred_at >= ?1
             GROUP BY tool_name
             ORDER BY COUNT(*) DESC, tool_name ASC",
        )?;
        let by_tool: Vec<ToolBreakdown> = stmt
            .query_map(params![cutoff], |row| {
                Ok(ToolBreakdown {
                    tool_name: row.get(0)?,
                    count: row.get(1)?,
                    avg_latency_ms: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        let last_call_at: Option<String> = conn
            .query_row(
                "SELECT occurred_at FROM mcp_call_log ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        Ok(McpCallStats24h {
            total,
            success,
            failed,
            avg_latency_ms: avg_latency_ms.unwrap_or(0.0),
            by_tool,
            last_call_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_recent_round_trip() {
        let db = Db::open_in_memory().unwrap();
        db.insert_mcp_call("get_project_context", 12, true, None)
            .unwrap();
        db.insert_mcp_call("search_memory", 30, true, None).unwrap();
        db.insert_mcp_call("get_session", 5, false, Some("session not found"))
            .unwrap();

        let rows = db.recent_mcp_calls(10).unwrap();
        assert_eq!(rows.len(), 3);
        // 应当按 id DESC，最后写入的在前
        assert_eq!(rows[0].tool_name, "get_session");
        assert!(!rows[0].success);
        assert_eq!(rows[0].error_message.as_deref(), Some("session not found"));
        assert_eq!(rows[2].tool_name, "get_project_context");
        assert!(rows[2].success);
    }

    #[test]
    fn limit_zero_returns_empty() {
        let db = Db::open_in_memory().unwrap();
        db.insert_mcp_call("foo", 1, true, None).unwrap();
        let rows = db.recent_mcp_calls(0).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn stats_24h_aggregates_by_tool() {
        let db = Db::open_in_memory().unwrap();
        db.insert_mcp_call("search_memory", 100, true, None)
            .unwrap();
        db.insert_mcp_call("search_memory", 200, true, None)
            .unwrap();
        db.insert_mcp_call("search_memory", 300, true, None)
            .unwrap();
        db.insert_mcp_call("get_session", 10, true, None).unwrap();
        db.insert_mcp_call("get_session", 0, false, Some("not found"))
            .unwrap();

        let stats = db.mcp_call_stats_24h().unwrap();
        assert_eq!(stats.total, 5);
        assert_eq!(stats.success, 4);
        assert_eq!(stats.failed, 1);
        // avg 只看 success=1 的样本: (100+200+300+10)/4 = 152.5
        assert!((stats.avg_latency_ms - 152.5).abs() < 0.01);

        assert_eq!(stats.by_tool.len(), 2);
        // ORDER BY COUNT(*) DESC, tool_name ASC → 调用次数多的在前
        assert_eq!(stats.by_tool[0].tool_name, "search_memory");
        assert_eq!(stats.by_tool[0].count, 3);
        assert!((stats.by_tool[0].avg_latency_ms - 200.0).abs() < 0.01);
        assert_eq!(stats.by_tool[1].tool_name, "get_session");
        assert_eq!(stats.by_tool[1].count, 2);

        assert!(
            stats.last_call_at.is_some(),
            "last_call_at should be populated"
        );
    }

    /// count 相等时按 tool_name ASC 二级排序，保证 UI 渲染顺序可预测。
    #[test]
    fn stats_24h_tie_break_by_tool_name() {
        let db = Db::open_in_memory().unwrap();
        db.insert_mcp_call("zebra", 10, true, None).unwrap();
        db.insert_mcp_call("apple", 20, true, None).unwrap();

        let stats = db.mcp_call_stats_24h().unwrap();
        assert_eq!(stats.by_tool.len(), 2);
        assert_eq!(stats.by_tool[0].tool_name, "apple");
        assert_eq!(stats.by_tool[1].tool_name, "zebra");
    }

    #[test]
    fn stats_24h_empty_table_returns_zero() {
        let db = Db::open_in_memory().unwrap();
        let stats = db.mcp_call_stats_24h().unwrap();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.success, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.avg_latency_ms, 0.0);
        assert!(stats.by_tool.is_empty());
        assert!(stats.last_call_at.is_none());
    }
}
