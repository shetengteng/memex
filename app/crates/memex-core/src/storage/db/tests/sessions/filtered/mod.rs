//! `list_sessions_filtered_paged` —— 资料库多维过滤。
//!
//! 配套 commit 修复的 bug：facets 显示 codex 13 个会话但勾选后 0 个，
//! 因为 counts 走全表 `stats_breakdown`，而 filter 走前端内存里只有 200
//! 条最近会话。修复后所有 filter 维度都下推到 SQL，前端不再做内存过滤。
//!
//! 子模块按"测试主题"切：
//!   * `basic` —— 空 filter / adapter / project / 排序：参数化、不依赖 L2 摘要。
//!   * `dimensions` —— time / summary / query / composite：需要写 L2 摘要或
//!     update_session_intent 等"额外副作用"，独立成块避免和基础读取混在一起。

mod basic;
mod dimensions;

use crate::storage::db::Db;

/// 给 list_sessions_filtered_paged 测试灌一条 session，可控时间戳 + 可控
/// 消息数。`message_count > 0` 是为了绕过 list_sessions_paged 自带的
/// "1 天内空会话" 过滤，让测试无需关心今天/昨天的相对时间。
///
/// 用 struct 而不是位置参数，避免 8 个位置参数触发 clippy::too_many_arguments，
/// 也让测试调用点像 named arguments 一样可读。
pub(super) struct SessionSeed<'a> {
    pub id: &'a str,
    pub source: &'a str,
    pub project_path: Option<&'a str>,
    pub title: Option<&'a str>,
    pub created_at: &'a str,
    pub updated_at: &'a str,
    pub message_count: i64,
}

pub(super) fn seed_filtered_session(db: &Db, s: SessionSeed<'_>) {
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO sessions (id, source, project_path, file_path, title, created_at, updated_at, message_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            s.id,
            s.source,
            s.project_path,
            format!("/{}.jsonl", s.id),
            s.title,
            s.created_at,
            s.updated_at,
            s.message_count,
        ],
    )
    .unwrap();
}
