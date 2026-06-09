//! `~/.cursor/hooks.json` 等配置文件的 JSON 读写以及"识别 memex 条目"的工具集。
//!
//! 三个 IDE 用相似但不完全相同的 JSON shape，所以 helpers 必须既能处理
//! Cursor 那种"扁平 entry 数组"，也能处理 Claude/Codex 那种"分组 entry 数组"。
//! 通过 `is_memex_command` 这个统一签名来识别 memex 安装的条目，
//! 卸载时只清自己装的，不动用户自己的 hook。

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

/// Cursor 风格的简单条目数组：`[{ "command": "<wrapper>" }, ...]`
pub(super) fn upsert_cmd_array(
    parent: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    wrapper: &Path,
) {
    let arr = parent
        .entry(key.to_string())
        .or_insert(serde_json::json!([]))
        .as_array_mut();
    if let Some(arr) = arr {
        arr.retain(|entry| {
            entry
                .get("command")
                .and_then(|c| c.as_str())
                .map(|s| !is_memex_command(s))
                .unwrap_or(true)
        });
        arr.push(serde_json::json!({
            "command": wrapper.to_string_lossy()
        }));
    }
}

pub(super) fn remove_memex_entries(
    parent: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) {
    if let Some(arr) = parent.get_mut(key).and_then(|a| a.as_array_mut()) {
        arr.retain(|entry| {
            entry
                .get("command")
                .and_then(|c| c.as_str())
                .map(|s| !is_memex_command(s))
                .unwrap_or(true)
        });
    }
}

/// Claude/Codex 风格的分组数组：
/// `[ { "matcher": "...", "hooks": [ { "type":"command", "command":"..." } ] }, ... ]`
pub(super) fn purge_memex_grouped(arr: &mut Vec<serde_json::Value>) {
    arr.retain(|group| {
        let Some(inner) = group.get("hooks").and_then(|x| x.as_array()) else {
            return true;
        };
        !inner.iter().any(|h| {
            h.get("command")
                .and_then(|c| c.as_str())
                .map(is_memex_command)
                .unwrap_or(false)
        })
    });
}

pub(super) fn has_memex_command(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Array(arr) => arr.iter().any(has_memex_command),
        serde_json::Value::Object(o) => o
            .get("command")
            .and_then(|c| c.as_str())
            .map(is_memex_command)
            .unwrap_or(false),
        _ => false,
    }
}

/// memex 写入的 wrapper 都在 `~/.memex/hooks/*-session-start.sh`，
/// 这是足够稳定的识别签名 —— 卸载时按此 prefix 精确剔除，不动其它用户 hook。
pub(super) fn is_memex_command(cmd: &str) -> bool {
    cmd.contains("/.memex/hooks/")
        || cmd.contains("/memex/hooks/")
        || cmd.ends_with("memex-session-start.sh")
}

pub(super) fn read_json_or_empty(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    Ok(serde_json::from_str(&content).unwrap_or(serde_json::json!({})))
}

pub(super) fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let out = serde_json::to_string_pretty(value)?;
    fs::write(path, out).with_context(|| format!("failed to write {}", path.display()))
}
