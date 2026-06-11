//! stdio JSON-RPC 传输层：逐行读 stdin，写 stdout，单行 JSON 消息。
//!
//! 5c 起 server 不再持有 `&Db`；所有数据访问由 [`McpClient`] 通过 daemon
//! HTTP 完成。`run_stdio` 内不负责 connect —— caller（memex-cli mcp 子命令）
//! 已经在 connect 失败时给出 user-facing 错误，传进来时一定是健康的 client。

use std::io::{self, BufRead, Write};

use anyhow::Result;

use super::dispatch::handle_request;
use crate::client::McpClient;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

pub fn run_stdio(client: &McpClient) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(None, -32700, &format!("parse error: {}", e));
                write_response(&mut out, &resp)?;
                continue;
            }
        };

        let response = handle_request(&request, client);
        write_response(&mut out, &response)?;
    }

    Ok(())
}

fn write_response(out: &mut impl Write, resp: &JsonRpcResponse) -> Result<()> {
    let json = serde_json::to_string(resp)?;
    writeln!(out, "{}", json)?;
    out.flush()?;
    Ok(())
}
