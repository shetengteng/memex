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

/// IPC 序列化形态使用 `#[serde(rename_all = "camelCase")]`，与 JS 命名习惯
/// 对齐（出去的 JSON 是 `projectPath` / `messageCount` / `createdAt` 等）。
///
/// 同时这条结构是 `Deserialize` 的，因为 `serde_rusqlite::from_rows` 走它
/// 把 SQL 行映射成 SessionRow —— **SQL 列名仍是 snake_case**。为此每个
/// 多词字段都补了 `#[serde(alias = "<snake_case>")]`，让 deserializer 既
/// 接受新的 camelCase（用于 JSON 输入），也接受历史 snake_case（用于 SQL
/// 列）。
///
/// 锁定形态见 `storage::json_contract_tests::test_session_row_json_fields`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: String,
    pub source: String,
    #[serde(alias = "project_path")]
    pub project_path: Option<String>,
    pub title: Option<String>,
    #[serde(alias = "message_count")]
    pub message_count: i64,
    #[serde(alias = "created_at")]
    pub created_at: String,
    #[serde(alias = "updated_at")]
    pub updated_at: String,
    /// L2 摘要的标题（已经镜像写到了 `sessions.title`，但单独保留一份方便
    /// UI 区分"原始来源标题"和"LLM 生成的标题"）。当前与 `title` 同值，
    /// 预留供后续拆分使用。
    #[serde(alias = "summary_title")]
    pub summary_title: Option<String>,
    /// 第一条 user 消息的预览（约 120 字），尚未生成摘要时作为 fallback，
    /// 避免 popup 列表里整条目为空。
    #[serde(alias = "first_user_message")]
    pub first_user_message: Option<String>,
    /// L2 摘要中由 LLM 推断出的"用户真实意图"，一句话。
    /// 摘要尚未生成时为 None；UI 在列表里用它代替原始首条消息预览。
    pub intent: Option<String>,
}

/// IPC 出去的会话详情，`#[serde(rename_all = "camelCase")]` 让多词字段在
/// JSON 里成 `projectPath` / `filePath` / `createdAt` 等。**只 Serialize**,
/// 不需要 alias —— 没有任何路径会从 JSON 反序列化回这个 struct。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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

/// 单条会话消息。当前所有字段都是单词（id / role / content / timestamp），
/// rename_all camelCase 与 snake_case 等价；显式加 attr 是把契约写在结构
/// 上而不是隐含在"碰巧没多词"上 —— 未来加多词字段（如 `tool_calls`）时
/// 不会忘。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: Option<String>,
}

/// 资料库列表多维筛选。所有字段都是可选——None / 空 Vec / 无法识别的字符串
/// 都按"不过滤"处理，绝不会回退到"删空集"，避免前端因为传错值看到 0 行而
/// 误以为是 bug。
///
/// 字段命名对齐前端 `LibraryFacets.vue` + `sessionFilters.ts`：
///   * `adapters` — 多选 source 值（如 "claude_code" / "cursor"）
///   * `projects` — 多选完整 `project_path`（如 "/Users/me/repo/memex"），
///     后端用 `IN (?, ?, ...)` 精确匹配；前端 `LibraryFacets.vue` 持有完整
///     路径并对同末段路径做去歧义显示，避免不同前缀的同名子目录被混算
///     （如 `/A/src` 和 `/B/src` 都被视作 "src"）
///   * `time` — "today" / "7d" / "30d" / "90d" / "all"
///   * `summary` — "all" / "done" / "pending"（done = 已生成 L2 摘要）
///   * `query` — 在 title / intent / L2 summary title / 首条 user message
///     上做 `LIKE %q%`
///   * `sort` — "recent" (默认) / "duration" / "messages"
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionListFilter {
    pub adapters: Option<Vec<String>>,
    pub projects: Option<Vec<String>>,
    pub time: Option<String>,
    pub summary: Option<String>,
    pub query: Option<String>,
    pub sort: Option<String>,
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
