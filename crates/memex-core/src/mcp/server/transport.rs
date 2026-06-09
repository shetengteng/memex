//! stdio JSON-RPC 传输层：逐行读 stdin，写 stdout，单行 JSON 消息。

use std::io::{self, BufRead, Write};

use anyhow::Result;

use super::dispatch::handle_request;
use crate::mcp::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::storage::db::Db;

pub fn run_stdio(db: &Db) -> Result<()> {
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

        let response = handle_request(&request, db);
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
