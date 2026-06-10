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
///
/// **序列化契约**：`#[serde(rename_all = "camelCase")]` 让 IPC 出去的 JSON 字段
/// 是 `sessionCount` / `createdAt` / `firstSessionAt` 等 camelCase 形态，
/// 与 JS 生态命名习惯对齐。Rust 侧字段保持 snake_case 以符合 Rust 风格。
/// 见 `serializes_with_camel_case` 测试锁定该形态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
///
/// 字段 `thread` / `sessions` 都是单词，camelCase 与 snake_case 形态相同，但
/// 仍显式加 attr 以让契约意图与 ThreadRow 一致，**未来增加多词字段时不会忘**。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDetail {
    pub thread: ThreadRow,
    pub sessions: Vec<SessionRow>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 锁定 ThreadRow 的 IPC 序列化形态：所有多词字段为 camelCase，
    /// snake_case 字段在 JSON 里不应再出现。前端 `tauri-app/src/types/index.ts`
    /// 的 `ThreadRow` 接口必须与本测试断言保持一致。
    #[test]
    fn thread_row_serializes_with_camel_case() {
        let row = ThreadRow {
            id: 7,
            name: "Tauri 多窗口".into(),
            summary: "讨论 Tauri 多窗口踩坑".into(),
            session_count: 3,
            created_at: "2026-06-08T10:00:00+00:00".into(),
            updated_at: "2026-06-08T11:00:00+00:00".into(),
            first_session_at: Some("2026-06-01T10:00:00+00:00".into()),
            last_session_at: Some("2026-06-08T10:00:00+00:00".into()),
            projects: vec!["/Users/me/proj".into()],
            adapters: vec!["cursor".into()],
        };
        let v = serde_json::to_value(&row).unwrap();

        assert_eq!(v["sessionCount"], 3);
        assert_eq!(v["createdAt"], "2026-06-08T10:00:00+00:00");
        assert_eq!(v["updatedAt"], "2026-06-08T11:00:00+00:00");
        assert_eq!(v["firstSessionAt"], "2026-06-01T10:00:00+00:00");
        assert_eq!(v["lastSessionAt"], "2026-06-08T10:00:00+00:00");

        assert!(v.get("session_count").is_none(), "snake_case must not leak");
        assert!(v.get("created_at").is_none(), "snake_case must not leak");
        assert!(v.get("updated_at").is_none(), "snake_case must not leak");
        assert!(
            v.get("first_session_at").is_none(),
            "snake_case must not leak"
        );
        assert!(
            v.get("last_session_at").is_none(),
            "snake_case must not leak"
        );
    }
}

/// LLM 聚类输出的一个 thread 草稿，准备落库。
#[derive(Debug, Clone)]
pub struct ThreadDraft {
    pub name: String,
    pub summary: String,
    pub session_ids: Vec<String>,
}
