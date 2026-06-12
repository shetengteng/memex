//! 项目工作记忆注入模块。
//!
//! 这一层把 SQLite 里现成的 `sessions` / `summaries` /
//! `aggregate_summaries` 拼装成一份给 AI 看的 Markdown 报告 ——
//! 仿照 TARS `tars inject` 的"工作记忆"格式 —— 然后通过 IDE 的
//! hook（Claude Code `SessionStart`、Cursor `sessionStart`）在
//! 会话启动时塞进系统上下文，让 AI 从第一轮就知道"在这个项目上
//! 之前做过什么 / 决定过什么 / 下一步要做什么"。
//!
//! 设计原则：
//! - 这个模块只**读** DB，**不**触发 LLM —— hook 不能慢
//! - 模糊匹配纯函数化，方便测试
//! - 输出格式跟 IDE hook 协议**解耦**（hook wrapper 负责 JSON
//!   套壳）—— 命令本身永远只发 Markdown

pub mod builder;
pub mod matcher;

pub use builder::{ContextOptions, build_context};
pub use matcher::{MatchTier, ProjectMatch, search_by_project};
