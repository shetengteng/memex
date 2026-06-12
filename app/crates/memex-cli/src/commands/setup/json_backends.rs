//! Cursor / Claude Code / OpenCode 都用 JSON 存 MCP 配置，但顶层 key 不一样：
//! - Cursor & Claude Code：`mcpServers` → `{ command, args }`
//! - OpenCode：           `mcp`        → `{ type, command:[bin, ...args], enabled }`
//!
//! upsert 总是覆盖整条 entry —— 调用方只关心「memex」server 是否最新；不
//! 试图与用户在同一个 entry 下加的额外字段做精细合并。

use std::path::Path;

use anyhow::{Context, Result};

use super::io::{read_json_or_empty, write_json};

pub(super) fn json_command_entry(memex_bin: &Path) -> serde_json::Value {
    serde_json::json!({
        "command": memex_bin.to_string_lossy(),
        "args": ["mcp"]
    })
}

pub(super) fn upsert_json_mcp_servers(
    path: &Path,
    server_name: &str,
    entry: serde_json::Value,
) -> Result<()> {
    let mut config = read_json_or_empty(path)?;
    let obj = config
        .as_object_mut()
        .context("config file is not a JSON object")?;
    let servers = obj
        .entry("mcpServers".to_string())
        .or_insert(serde_json::json!({}));
    let servers = servers
        .as_object_mut()
        .context("mcpServers is not a JSON object")?;
    servers.insert(server_name.to_string(), entry);
    write_json(path, &config)
}

pub(super) fn remove_json_mcp_servers(path: &Path, server_name: &str) -> Result<()> {
    let mut config = read_json_or_empty(path)?;
    let Some(obj) = config.as_object_mut() else {
        return Ok(());
    };
    if let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.remove(server_name);
    }
    write_json(path, &config)
}

pub(super) fn probe_json_mcp_servers(content: &str, server_name: &str) -> (bool, Option<String>) {
    let Ok(v): serde_json::Result<serde_json::Value> = serde_json::from_str(content) else {
        return (false, None);
    };
    let Some(entry) = v.get("mcpServers").and_then(|s| s.get(server_name)) else {
        return (false, None);
    };
    let cmd = entry
        .get("command")
        .and_then(|v| v.as_str())
        .map(String::from);
    (true, cmd)
}

pub(super) fn upsert_opencode_json(path: &Path, server_name: &str, memex_bin: &Path) -> Result<()> {
    let mut config = read_json_or_empty(path)?;
    let obj = config
        .as_object_mut()
        .context("config file is not a JSON object")?;
    obj.entry("$schema".to_string())
        .or_insert(serde_json::Value::String(
            "https://opencode.ai/config.json".to_string(),
        ));
    let mcp = obj
        .entry("mcp".to_string())
        .or_insert(serde_json::json!({}));
    let mcp = mcp.as_object_mut().context("mcp is not a JSON object")?;
    mcp.insert(
        server_name.to_string(),
        serde_json::json!({
            "type": "local",
            "command": [memex_bin.to_string_lossy(), "mcp"],
            "enabled": true,
        }),
    );
    write_json(path, &config)
}

pub(super) fn remove_opencode_json(path: &Path, server_name: &str) -> Result<()> {
    let mut config = read_json_or_empty(path)?;
    let Some(obj) = config.as_object_mut() else {
        return Ok(());
    };
    if let Some(mcp) = obj.get_mut("mcp").and_then(|v| v.as_object_mut()) {
        mcp.remove(server_name);
    }
    write_json(path, &config)
}

pub(super) fn probe_opencode_json(content: &str, server_name: &str) -> (bool, Option<String>) {
    let Ok(v): serde_json::Result<serde_json::Value> = serde_json::from_str(content) else {
        return (false, None);
    };
    let Some(entry) = v.get("mcp").and_then(|s| s.get(server_name)) else {
        return (false, None);
    };
    let cmd = entry
        .get("command")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(String::from);
    (true, cmd)
}
