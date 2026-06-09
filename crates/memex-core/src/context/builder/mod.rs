//! 把 SQLite 里的项目记忆拼装成一份给 AI 看的 Markdown 上下文。
//!
//! 输出格式向 TARS `tars inject` 看齐（用户在文档里明确说"参考一下
//! tars"），但内容来源完全是 Memex 自己的表：
//!
//! - 项目概览 / 总会话数 / 最近活跃 → `sessions` 表聚合
//! - L3 项目级摘要 → `aggregate_summaries (scope_type='project')`
//! - 最近 N 个会话的 L2 摘要 → `summaries (level='L2_session')`
//! - "最后提示" → 每个 session 的第一条 user 消息
//!
//! 脱敏：跟随 `config.privacy.redaction_enabled`。打开时，最终输出的
//! Markdown 文本走一遍 redact 规则，避免把 token / 密码片段塞进
//! 任何外部 LLM 的会话上下文（这一点比直接打印 raw 文本更安全）。
//!
//! 子模块按 pipeline 阶段拆：
//!   * `collect` —— 从 DB 拉数据 + 信号过滤 + 装配 `ProjectContext`，
//!     以及对外的 `build_context` 入口（collect + render + redact）。
//!   * `render`  —— 把 `ProjectContext` 渲染成 Markdown 字符串。

mod collect;
mod render;
mod text;

use serde::{Deserialize, Serialize};

pub use collect::{build_context, collect_project_context};
pub use render::render_markdown;

#[derive(Debug, Clone)]
pub struct ContextOptions {
    /// 最多列多少个最近会话。TARS 默认 2-3，我们默认 3，让 AI 既能看到
    /// 当下任务又能感知近期上下文。
    pub top_n: usize,
    /// 是否在最终输出上应用 redaction 规则。一般跟随
    /// `config.privacy.redaction_enabled`；CLI 也允许显式覆盖。
    pub redact: bool,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            top_n: 3,
            redact: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub project_path: String,
    pub project_name: String,
    pub total_sessions: i64,
    pub last_active: Option<String>,
    pub project_summary: Option<String>,
    pub project_topics: Vec<String>,
    pub recent_sessions: Vec<SessionContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub adapter: String,
    pub updated_at: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    /// L2 摘要推断出的"用户真实意图"，渲染时优先于 first_user_message。
    pub intent: Option<String>,
    pub first_user_message: Option<String>,
}

#[cfg(test)]
pub(super) mod test_support {
    //! 共享的测试 seed helper。collect / render 两个子模块的测试都依赖
    //! 同一份 fixture（2 个 session + 4 条 message + L2 + L3 摘要），
    //! 放在这里避免重复定义。

    use crate::storage::db::{AggregateSummaryUpsert, Db, SummaryUpsert};

    pub fn seed(db: &Db, project_path: &str) {
        db.insert_session(
            "s1",
            "claude_code",
            Some(project_path),
            "/f1.jsonl",
            1717000000,
            1717010000,
        )
        .unwrap();
        db.insert_session(
            "s2",
            "cursor",
            Some(project_path),
            "/f2.jsonl",
            1717100000,
            1717110000,
        )
        .unwrap();
        let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
        // s1: 第一条 user 消息 + 2 条 assistant，便于触发 message_count >= 2
        db.insert_message("m1", "s1", "user", "fix the login bug", None, 0, &h("a"))
            .unwrap();
        db.insert_message("m2", "s1", "assistant", "ok", None, 1, &h("b"))
            .unwrap();
        db.insert_message("m3", "s2", "user", "design the migration", None, 0, &h("c"))
            .unwrap();
        db.insert_message("m4", "s2", "assistant", "ack", None, 1, &h("d"))
            .unwrap();

        db.upsert_summary(SummaryUpsert {
            session_id: "s1",
            level: "L2_session",
            title: Some("Fix login bug"),
            summary: "Fixed the JWT parser when audience claim is missing.",
            topics: &["auth".into(), "jwt".into()],
            decisions: &["use RS256".into()],
            message_count_at_creation: 2,
        })
        .unwrap();

        db.upsert_aggregate_summary(AggregateSummaryUpsert {
            scope_type: "project",
            scope_key: project_path,
            title: Some("My Project"),
            summary: "Project-wide work on Memex —— Rust + Tauri + Vue.",
            topics: &["memex".into(), "rust".into()],
            decisions: &[],
            session_count: 5,
        })
        .unwrap();
    }
}
