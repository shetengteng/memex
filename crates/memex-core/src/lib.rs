//! Memex 本地优先跨 LLM 会话记忆中枢的核心库。
//!
//! 这个 crate 提供：
//! - 多 IDE 会话采集（Claude Code / Cursor / Codex / OpenCode / Aider / Cline /
//!   Continue Dev 等，见 [`collector`]）
//! - SQLite 持久化与 FTS5 全文检索（[`storage`] / [`retriever`]）
//! - LLM 驱动的会话摘要 / 反思 / 聚类（[`llm`] / [`reflect`]）
//! - MCP server 实现，把记忆 expose 给 AI agent（[`mcp`]）
//! - 上下文渲染、会话脱敏、隐私过滤（[`context`] / [`processor`]）
//!
//! 这是一个 library，不会自己运行。可执行入口在 `memex-cli` / `memex-daemon`
//! / `memex-menubar` 三个 binary crate 里。

#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

pub mod collector;
pub mod config;
pub mod context;
pub mod ingest;
pub mod llm;
pub mod maintenance;
pub mod processor;
pub mod reflect;
pub mod retriever;
pub mod storage;

use std::path::PathBuf;

/// 解析 Memex 的工作目录。
///
/// 如果设置了 `MEMEX_HOME` 环境变量，优先使用它（方便测试、CI、多用户场景，
/// 或者在同一台机器上并排跑多个 Memex 实例）。
/// 否则回退到 `~/.memex`。
pub fn memex_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("MEMEX_HOME")
        && !custom.trim().is_empty()
    {
        return PathBuf::from(custom);
    }
    // INVARIANT: HOME (or platform equivalent) is always set in the
    // environments memex targets (macOS, Linux, Windows desktop). The CLI is
    // unusable without it, so panicking here is honest.
    dirs::home_dir()
        .expect("INVARIANT: home directory must be resolvable")
        .join(".memex")
}
