//! `threads` 表与 `thread_sessions` 关联表的 DTO + 草稿结构。
//!
//! `ThreadRow` / `ThreadDetail` 序列化给前端；`ThreadDraft` 是 LLM 聚类
//! 输出的入参，专门跟 `upsert_thread_with_sessions` 配对。

use serde::Serialize;

use crate::storage::db::sessions::SessionRow;

/// `threads` 表的一行 + 用于 list_threads 的展示信息。
/// 只需 Serialize（IPC 出去给前端），Deserialize 不需要——
/// 来自 LLM 的 thread 草稿走 ThreadDraft，不复用此类型。
///
/// 卡片视图额外需要的派生字段（`first_session_at` / `last_session_at` /
/// `projects` / `adapters`）通过 list_threads SQL 一次 JOIN 聚合返回，
/// 避免前端 N+1。
#[derive(Debug, Clone, Serialize)]
pub struct ThreadRow {
    pub id: i64,
    pub name: String,
    pub summary: String,
    pub session_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// 命中会话中最早的 created_at（按所有 sessions 时间跨度）。可能为空（没有命中或全部 session 已删）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_session_at: Option<String>,
    /// 命中会话中最晚的 updated_at。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_session_at: Option<String>,
    /// 涉及的项目（去重后用 `\n` 串联，前端 split），便于一次返回数组。
    /// 用 `\n` 而不是 `,` 因为项目路径里有逗号的可能比换行多。
    #[serde(default)]
    pub projects: Vec<String>,
    /// 涉及的适配器（去重后），如 cursor / claude_code。
    #[serde(default)]
    pub adapters: Vec<String>,
}

/// 一个 thread 的详情：基础信息 + 命中的 session 列表（取 SessionRow 给前端复用）。
#[derive(Debug, Clone, Serialize)]
pub struct ThreadDetail {
    pub thread: ThreadRow,
    pub sessions: Vec<SessionRow>,
}

/// LLM 聚类输出的一个 thread 草稿，准备落库。
#[derive(Debug, Clone)]
pub struct ThreadDraft {
    pub name: String,
    pub summary: String,
    pub session_ids: Vec<String>,
}
