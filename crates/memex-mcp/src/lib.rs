//! Memex 的 MCP（Model Context Protocol）server 实现。
//!
//! 把 [`memex_core`] 里持久化的会话记忆通过 MCP 协议 expose 给 AI agent
//! （Claude Code / Cursor / Codex / OpenCode / 等）。运行方式是从 stdio
//! 读 JSON-RPC 请求、写回 JSON-RPC 响应：caller 只需一行
//!
//! ```ignore
//! memex_mcp::server::run_stdio(&db)?;
//! ```
//!
//! 拆分自 `memex-core::mcp`（v0.3.4 之前）—— 当时 4 个文件 + tests 共
//! 781 行全部在 memex-core 内，导致 mcp 改动重编 core 全套测试。独立
//! 后只有 cli 一个 caller，改动隔离。
//!
//! ## 模块布局
//!
//! - [`protocol`] — JSON-RPC 类型 + tool 名常量
//! - [`server`]   — stdio transport + dispatch + tool 实现

#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

pub mod client;
pub mod protocol;
pub mod server;

pub use client::McpClient;

#[cfg(test)]
mod tests;
