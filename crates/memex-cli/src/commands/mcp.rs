//! `memex mcp` —— 把 daemon 暴露成 IDE 端 MCP server。
//!
//! 5c 起 mcp 不直连 db：先 connect 到 daemon HTTP，再把 client 句柄丢给
//! [`memex_mcp::server::run_stdio`]。daemon 没起来时直接 bail user-facing
//! 错误，IDE 端会看到 stderr 上的 friendly message。

use anyhow::Result;

use memex_mcp::{McpClient, server};

pub fn run() -> Result<()> {
    let client = McpClient::connect()?;
    server::run_stdio(&client)
}
