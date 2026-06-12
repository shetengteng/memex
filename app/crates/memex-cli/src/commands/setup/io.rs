//! 读写 JSON / TOML 配置文件的小工具集。
//!
//! 失败时一律降级到「空配置」而不是 propagate —— 用户机器上很常见 IDE
//! 配置文件破损（手动改坏），我们的目标是把 `memex` 这条 server entry 写进去，
//! 不应该被无关字段拖垮。

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub(super) fn read_json_or_empty(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&content).or_else(|_| Ok(serde_json::json!({})))
}

pub(super) fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let output = serde_json::to_string_pretty(value)?;
    fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn read_toml_or_empty(path: &Path) -> Result<toml::Value> {
    if !path.exists() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    toml::from_str(&content).or_else(|_| Ok(toml::Value::Table(toml::map::Map::new())))
}

pub(super) fn write_toml(path: &Path, value: &toml::Value) -> Result<()> {
    let output = toml::to_string_pretty(value)?;
    fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))
}
