//! `memex ingest` —— Phase 5b-3 起完全走 daemon RPC。
//!
//! 设计取舍：之所以让 CLI 走 RPC 而不是直连 db，是因为 daemon 启动时会跑一
//! 次 bootstrap ingest，再加上 watcher 持续监听新文件。如果 CLI 还能绕过
//! daemon 直接写 db，会出现两个写入路径同时持有 SQLite WAL 的窗口，触发
//! "database is locked"。统一通道之后，CLI 和 daemon 看到的是同一份数据。
//!
//! 因为 ingest 可能跑数十秒甚至几分钟（首次扫 cursor + claude_code），客户端
//! 用 `post_long`（10 min timeout），避免在 release 模式下被 30s default
//! timeout 误杀。

use anyhow::Result;
use serde::Serialize;

use crate::client::MemexClient;

#[derive(Serialize)]
struct IngestBody<'a> {
    adapter: Option<&'a str>,
}

pub fn run(adapter_filter: Option<&str>, json: bool) -> Result<()> {
    let client = MemexClient::connect()?;
    let body = IngestBody {
        adapter: adapter_filter,
    };
    let resp: serde_json::Value = client.post_long("/ingest", &body)?;

    let messages = resp
        .get("messages_ingested")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let chunks = resp
        .get("chunks_created")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if json {
        crate::io::json(&serde_json::json!({
            "messages_ingested": messages,
            "chunks_created": chunks,
        }))?;
    } else {
        crate::out!("Ingested {} messages, created {} chunks", messages, chunks);
    }
    Ok(())
}
