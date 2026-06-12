//! `notifications` 表的写入与查询。
//!
//! 用户在 Settings 里能开启 3 类通知（`pref.notify.<kind>`）：
//!
//! * `weekly_report`    —— 周日 22:00 周报生成完成
//! * `reflect_pending`  —— 反思待处理超过 24 小时
//! * `ingest_failed`    —— 采集源同步失败（无法解析某个会话）
//!
//! 后端触发某个事件时调 [`Db::insert_notification`] 写一行，前端通过
//! [`Db::list_notifications`] 拉最近 N 条。未读判定靠 `read_at IS NULL`。
//!
//! 数据保留策略：跟 mcp_call_log 一致，不主动清理。预计量级一周几十条，
//! 对本地 db 可忽略。

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::db::Db;

pub const KIND_INGEST_FAILED: &str = "ingest_failed";
pub const KIND_SUMMARY_DONE: &str = "summary_done";
pub const KIND_REFLECT_PENDING: &str = "reflect_pending";
pub const KIND_WEEKLY_REPORT: &str = "weekly_report";

/// 单条通知的 IPC 形态。字段都用 snake_case，前端用 `useMemex` 拿到后做
/// 浅层包装。`payload_json` 是详情数据（任意 JSON 字符串），由 caller 决定语义。
#[derive(Debug, Clone, Serialize)]
pub struct NotificationEntry {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub payload_json: Option<String>,
    pub created_at: String,
    /// `None` 表示未读；`Some(iso8601)` 表示用户标记已读的时间。
    pub read_at: Option<String>,
}

impl Db {
    /// 插一行通知并返回 id。失败时返回 Err；触发方一般用 `let _ =` 吞错误，
    /// 避免通知写入失败影响主流程（例如 ingest 失败 → 通知插入失败 → 反而
    /// 让整个 ingest 卡住）。
    ///
    /// `payload_json` 传 `None` 表示无详情；传 `Some(s)` 时 caller 负责保证
    /// 是合法 JSON 字符串（写入层不做校验）。
    pub fn insert_notification(
        &self,
        kind: &str,
        title: &str,
        body: &str,
        payload_json: Option<&str>,
    ) -> Result<i64> {
        let created_at = self.now_utc().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO notifications (kind, title, body, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![kind, title, body, payload_json, created_at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 按 id 倒序取最近 N 条。`unread_only=true` 时只拉 `read_at IS NULL`。
    /// limit 上限 500，超出截断。
    pub fn list_notifications(&self, limit: usize, unread_only: bool) -> Result<Vec<NotificationEntry>> {
        let capped = limit.min(500) as i64;
        let conn = self.conn.lock();
        let sql = if unread_only {
            "SELECT id, kind, title, body, payload_json, created_at, read_at
             FROM notifications
             WHERE read_at IS NULL
             ORDER BY id DESC
             LIMIT ?1"
        } else {
            "SELECT id, kind, title, body, payload_json, created_at, read_at
             FROM notifications
             ORDER BY id DESC
             LIMIT ?1"
        };
        let mut stmt = conn.prepare_cached(sql)?;
        let rows = stmt
            .query_map(params![capped], |row| {
                Ok(NotificationEntry {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    body: row.get(3)?,
                    payload_json: row.get(4)?,
                    created_at: row.get(5)?,
                    read_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// 标记某条通知已读。已读的行再次标记是幂等的（read_at 不变）。
    /// 不存在的 id 返回 Ok(0)。
    pub fn mark_notification_read(&self, id: i64) -> Result<usize> {
        let read_at = self.now_utc().to_rfc3339();
        let conn = self.conn.lock();
        let n = conn.execute(
            "UPDATE notifications SET read_at = ?1
             WHERE id = ?2 AND read_at IS NULL",
            params![read_at, id],
        )?;
        Ok(n)
    }

    /// 一键全部已读。返回被标记的行数。
    pub fn mark_all_notifications_read(&self) -> Result<usize> {
        let read_at = self.now_utc().to_rfc3339();
        let conn = self.conn.lock();
        let n = conn.execute(
            "UPDATE notifications SET read_at = ?1
             WHERE read_at IS NULL",
            params![read_at],
        )?;
        Ok(n)
    }

    /// 当前未读通知数。前端 Bell badge 用这个值，3s 轮询。
    pub fn count_unread_notifications(&self) -> Result<i64> {
        let conn = self.conn.lock();
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notifications WHERE read_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_list_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let id1 = db.insert_notification(KIND_INGEST_FAILED, "采集失败", "解析 abc.jsonl 失败", None).unwrap();
        let id2 = db
            .insert_notification(
                KIND_SUMMARY_DONE,
                "摘要完成",
                "为 session xyz 生成 L1 摘要",
                Some(r#"{"session_id":"xyz"}"#),
            )
            .unwrap();
        assert!(id2 > id1);

        let rows = db.list_notifications(10, false).unwrap();
        assert_eq!(rows.len(), 2);
        // id DESC：后插入的在前
        assert_eq!(rows[0].kind, KIND_SUMMARY_DONE);
        assert_eq!(rows[0].payload_json.as_deref(), Some(r#"{"session_id":"xyz"}"#));
        assert_eq!(rows[1].kind, KIND_INGEST_FAILED);
        assert!(rows[0].read_at.is_none());
        assert!(rows[1].read_at.is_none());
    }

    #[test]
    fn mark_read_updates_only_target_row() {
        let db = Db::open_in_memory().unwrap();
        let id1 = db.insert_notification(KIND_INGEST_FAILED, "a", "b", None).unwrap();
        let id2 = db.insert_notification(KIND_INGEST_FAILED, "c", "d", None).unwrap();

        let n = db.mark_notification_read(id1).unwrap();
        assert_eq!(n, 1);

        let rows = db.list_notifications(10, false).unwrap();
        let read_state: Vec<(i64, bool)> = rows.iter().map(|r| (r.id, r.read_at.is_some())).collect();
        assert!(read_state.contains(&(id1, true)));
        assert!(read_state.contains(&(id2, false)));

        // 幂等：第二次标记同一条返回 0（read_at IS NULL 条件不满足）
        let n2 = db.mark_notification_read(id1).unwrap();
        assert_eq!(n2, 0);
    }

    #[test]
    fn unread_only_filters_correctly() {
        let db = Db::open_in_memory().unwrap();
        let id1 = db.insert_notification(KIND_INGEST_FAILED, "a", "b", None).unwrap();
        let _id2 = db.insert_notification(KIND_INGEST_FAILED, "c", "d", None).unwrap();
        db.mark_notification_read(id1).unwrap();

        let all = db.list_notifications(10, false).unwrap();
        assert_eq!(all.len(), 2);
        let unread = db.list_notifications(10, true).unwrap();
        assert_eq!(unread.len(), 1);
        assert!(unread[0].read_at.is_none());
    }

    #[test]
    fn count_unread_reflects_state() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.count_unread_notifications().unwrap(), 0);

        let id1 = db.insert_notification(KIND_INGEST_FAILED, "a", "b", None).unwrap();
        db.insert_notification(KIND_INGEST_FAILED, "c", "d", None).unwrap();
        assert_eq!(db.count_unread_notifications().unwrap(), 2);

        db.mark_notification_read(id1).unwrap();
        assert_eq!(db.count_unread_notifications().unwrap(), 1);

        db.mark_all_notifications_read().unwrap();
        assert_eq!(db.count_unread_notifications().unwrap(), 0);
    }

    #[test]
    fn limit_zero_returns_empty() {
        let db = Db::open_in_memory().unwrap();
        db.insert_notification(KIND_INGEST_FAILED, "a", "b", None).unwrap();
        let rows = db.list_notifications(0, false).unwrap();
        assert!(rows.is_empty());
    }
}
