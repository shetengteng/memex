//! 会话级别（session）的读写，以及 menubar IPC、MCP server、daemon HTTP API
//! 和 `memex session show` CLI 共同依赖的 `SessionRow` / `SessionDetail` /
//! `MessageRow` 数据结构。
//!
//! 子模块按读写分：
//!   * `read`  —— 列表、详情、计数、按 project / 时间范围的查询。
//!   * `write` —— 插入会话、回填 `project_path` / `intent`。

mod read;
mod write;

use serde::{Deserialize, Serialize};

/// `Deserialize` 是为了 `serde_rusqlite::from_rows` 的 row mapping —— SQL
/// 里 SELECT 出的列名必须和这里的字段名对齐（`summary_title` /
/// `first_user_message` 都已经在 SQL 里 `AS` 成了同名 alias）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// L2 摘要的标题（已经镜像写到了 `sessions.title`，但单独保留一份方便
    /// UI 区分"原始来源标题"和"LLM 生成的标题"）。当前与 `title` 同值，
    /// 预留供后续拆分使用。
    pub summary_title: Option<String>,
    /// 第一条 user 消息的预览（约 120 字），尚未生成摘要时作为 fallback，
    /// 避免 popup 列表里整条目为空。
    pub first_user_message: Option<String>,
    /// L2 摘要中由 LLM 推断出的"用户真实意图"，一句话。
    /// 摘要尚未生成时为 None；UI 在列表里用它代替原始首条消息预览。
    pub intent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionDetail {
    pub id: String,
    pub source: String,
    pub project_path: Option<String>,
    pub file_path: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
    pub messages: Vec<MessageRow>,
    /// 与 `SessionRow.intent` 同源：L2 摘要的"用户真实意图"。
    pub intent: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: Option<String>,
}

/// 新 session 的写入 payload。比起 7 个零散参数（`clippy::too_many_arguments`
/// 触发，规约 §6.2 也建议 builder/struct）显式构造一次，更便于 caller
/// 阅读与未来扩展（例如新增 `is_pinned` / `priority` 等元数据）。
pub struct NewSession<'a> {
    pub id: &'a str,
    pub source: &'a str,
    pub project_path: Option<&'a str>,
    pub file_path: &'a str,
    pub session_created_secs: u64,
    pub session_mtime_secs: u64,
    /// adapter 提供的"原始对话标题"（如 cursor composer.name、codex thread_name）。
    /// **仅在 sessions.title 当前为 NULL 时写入**：L2 摘要后续生成的更优 title
    /// 不会被这里覆盖。
    pub title: Option<&'a str>,
}
