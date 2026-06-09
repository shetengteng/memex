//! 消息写入。按 `(content_hash, session_id)` 做幂等去重；
//! 只有真的插入新行时才会自增 `sessions.message_count`。
//!
//! 注意：**不再在这里更新 `sessions.updated_at`**。
//! `updated_at` 完全由 `insert_session_with_title` 控制（基于 collector 上报
//! 的真实 mtime，例如 cursor `composer.last_updated_at` / claude_code jsonl
//! 文件 mtime / codex rollout 文件 mtime）。
//!
//! 历史 bug：之前每插入一条新消息都用 `Utc::now()` 覆盖 `sessions.updated_at`。
//! 这在 commit c57de98 修好 cursor 共享 file_path 之前并不显眼（大部分
//! 历史消息会被 last_offset 吞掉，UPDATE 不会被触发），修好后所有历史
//! 消息都重新走 insert_message，导致每条 cursor 会话的 updated_at 全部
//! 被刷成「今天」，把真实活动时间彻底覆盖。

use anyhow::Result;
use rusqlite::params;

use super::Db;

impl Db {
    #[allow(clippy::too_many_arguments)]
    pub fn insert_message(
        &self,
        id: &str,
        session_id: &str,
        role: &str,
        content: &str,
        timestamp: Option<&str>,
        source_offset: u64,
        content_hash: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE content_hash = ?1 AND session_id = ?2)",
                params![content_hash, session_id],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if exists {
            return Ok(false);
        }

        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, session_id, role, content, timestamp, source_offset, content_hash],
        )?;
        conn.execute(
            "UPDATE sessions SET message_count = message_count + 1 WHERE id = ?1",
            params![session_id],
        )?;
        Ok(true)
    }
}
