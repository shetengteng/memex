//! L2 摘要失败退避（`sessions.l2_attempts` / `sessions.l2_next_retry_at`）。
//!
//! 背景：早期 daemon 每 2 分钟一轮 ingest，同一批 session 因 LLM HTTP 错误
//! 反复失败，把 DB / WAL 锁住导致 MCP 30s 启动超时。本模块提供指数退避 +
//! 达到上限后永久跳过的机制。selector（[`Db::sessions_needing_summary`]）
//! 会带上 `l2_next_retry_at <= now` 的过滤条件。

use anyhow::Result;
use rusqlite::params;

use super::super::Db;

/// 达到该失败次数后，L2 摘要视为「永久失败」，selector 不再返回。
///
/// 5 次退避总时长 2+4+8+16+32=62 分钟，足够覆盖临时的 LLM 服务抖动；
/// 之后仍未成功大概率是坏数据 / 配置错误，人工介入。
pub const L2_MAX_ATTEMPTS: i64 = 5;

/// 永久放弃哨兵：字典序最大的 RFC3339 时间戳，selector 的 `<= now`
/// 条件永远不成立。
const NEVER_RETRY: &str = "9999-12-31T23:59:59Z";

impl Db {
    /// L2 摘要生成失败时调用：`l2_attempts += 1`，并根据当前 attempts 计算
    /// 下次可重试时间（指数退避）。
    ///
    /// 达到 [`L2_MAX_ATTEMPTS`] 后落一个远未来哨兵时间，等价于「永久放弃」，
    /// 避免每轮 ingest 都重试同一批坏 session 把 daemon 拖住。手动重生成走
    /// [`Self::reset_l2_backoff`] 清零。
    ///
    /// # Errors
    ///
    /// 底层 SQLite UPDATE 失败（如 DB 锁超时、只读挂载）时返回错误。
    /// `session_id` 不存在不算错误 —— UPDATE 影响 0 行、静默返回 Ok。
    pub fn record_l2_failure(&self, session_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        // 读当前 attempts 决定退避时长。COALESCE 保护老行 NULL 语义。
        let attempts: i64 = conn
            .query_row(
                "SELECT COALESCE(l2_attempts, 0) FROM sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let next_attempts = attempts + 1;
        let next_retry_at = if next_attempts >= L2_MAX_ATTEMPTS {
            NEVER_RETRY.to_string()
        } else {
            // 指数退避：2^attempts 分钟，上限 24h。attempts=1→2min，
            // 2→4min，3→8min，4→16min。
            let minutes = 2i64.saturating_pow(next_attempts as u32).min(24 * 60);
            (self.now_utc() + chrono::Duration::minutes(minutes)).to_rfc3339()
        };
        conn.execute(
            "UPDATE sessions
                SET l2_attempts = ?1, l2_next_retry_at = ?2
              WHERE id = ?3",
            params![next_attempts, next_retry_at, session_id],
        )?;
        Ok(())
    }

    /// L2 摘要成功一次或用户手动触发时调用：清零 attempts + 解除 `next_retry_at`，
    /// 让该 session 立即回到 selector 候选池。
    ///
    /// # Errors
    ///
    /// 底层 SQLite UPDATE 失败时返回错误。`session_id` 不存在不算错误。
    pub fn reset_l2_backoff(&self, session_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE sessions SET l2_attempts = 0, l2_next_retry_at = NULL WHERE id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    /// 用户主动触发批量摘要时调用：把所有已达上限（`l2_next_retry_at = NEVER_RETRY`）
    /// 的会话解除永久退避，重新进入 selector 候选池。
    /// 临时退避（有限时间）不动，让正常退避逻辑继续生效。
    ///
    /// # Errors
    ///
    /// 底层 SQLite UPDATE 失败时返回错误。
    pub fn reset_permanent_l2_backoff(&self) -> Result<u64> {
        let conn = self.conn.lock();
        let n = conn.execute(
            "UPDATE sessions SET l2_attempts = 0, l2_next_retry_at = NULL WHERE l2_next_retry_at = ?1",
            params![NEVER_RETRY],
        )?;
        Ok(n as u64)
    }
}
