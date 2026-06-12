//! `memex mcp` —— 把 daemon 暴露成 IDE 端 MCP server。
//!
//! Phase 7 起 memex-mcp 不再是独立 crate，而是作为 memex-cli 的子模块直接
//! 落地在 `commands/mcp/`：因为它唯一 caller 是 memex-cli 自己，独立 crate
//! 只是徒增 workspace 维护成本。
//!
//! 数据访问全部走 daemon HTTP RPC（5c 起）：先 connect 到本机 Memex 主进程
//! 内的 daemon，再把 client 句柄丢给 [`server::run_stdio`]。daemon 没起来时
//! 直接 bail 出 user-facing 错误，IDE 端会在 stderr 上看到 friendly message。
//!
//! ## 子模块布局
//!
//! - [`protocol`] —— JSON-RPC 类型 + tool 名常量
//! - [`client`]   —— 给 mcp 用的 daemon HTTP client
//! - [`server`]   —— stdio transport + dispatch + tool 实现

mod client;
mod protocol;
mod server;

#[cfg(test)]
mod tests;

use anyhow::Result;

pub fn run() -> Result<()> {
    let client = client::McpClient::connect()?;
    server::run_stdio(&client)
}
