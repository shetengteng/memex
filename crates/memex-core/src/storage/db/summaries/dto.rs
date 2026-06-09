//! L1 / L2 / L3 / L4 摘要表的 DTO + upsert payload。
//!
//! 序列化目标：通过 IPC 返回给前端。Upsert 结构同样按 named-field 写入
//! 是为了避免 `clippy::too_many_arguments` —— 7 个零散参数容易写错顺序。

use serde::Serialize;

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
