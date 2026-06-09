//! Codex 用 TOML 存 MCP servers，顶层是 `[mcp_servers.<name>]` 表。
//! 跟 JSON 后端的语义保持一致：upsert 覆盖 entry，remove 幂等，probe 提取
//! 当前 `command`。

use std::path::Path;

use anyhow::{Context, Result};

use super::io::{read_toml_or_empty, write_toml};

pub(super) fn upsert_codex_toml(path: &Path, server_name: &str, memex_bin: &Path) -> Result<()> {
    let mut doc = read_toml_or_empty(path)?;
    let root = doc
        .as_table_mut()
        .context("codex config root is not a table")?;
    let servers = root
        .entry("mcp_servers".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let servers = servers
        .as_table_mut()
        .context("[mcp_servers] is not a table")?;
    let mut entry = toml::map::Map::new();
    entry.insert(
        "command".to_string(),
        toml::Value::String(memex_bin.to_string_lossy().to_string()),
    );
    entry.insert(
        "args".to_string(),
        toml::Value::Array(vec![toml::Value::String("mcp".to_string())]),
    );
    entry.insert("enabled".to_string(), toml::Value::Boolean(true));
    servers.insert(server_name.to_string(), toml::Value::Table(entry));
    write_toml(path, &doc)
}

pub(super) fn remove_codex_toml(path: &Path, server_name: &str) -> Result<()> {
    let mut doc = read_toml_or_empty(path)?;
    if let Some(root) = doc.as_table_mut()
        && let Some(servers) = root.get_mut("mcp_servers").and_then(|v| v.as_table_mut())
    {
        servers.remove(server_name);
    }
    write_toml(path, &doc)
}

pub(super) fn probe_codex_toml(content: &str, server_name: &str) -> (bool, Option<String>) {
    let Ok(v): std::result::Result<toml::Value, _> = toml::from_str(content) else {
        return (false, None);
    };
    let Some(entry) = v.get("mcp_servers").and_then(|s| s.get(server_name)) else {
        return (false, None);
    };
    let cmd = entry
        .get("command")
        .and_then(|v| v.as_str())
        .map(String::from);
    (true, cmd)
}
