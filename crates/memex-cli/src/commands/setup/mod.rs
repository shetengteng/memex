//! `memex setup <ide>` —— 把 memex 注册为 IDE 的 MCP server。
//!
//! 4 个 IDE 走 3 套配置格式，所以拆成 3 个 backend 子模块：
//! - `json_backends` —— Cursor / Claude Code（`mcpServers`） + OpenCode（`mcp`）
//! - `codex_backend` —— Codex 的 TOML `mcp_servers`
//! - `io` —— 容错的 JSON / TOML 读写 helpers
//!
//! 本文件只保留对外 API（`run` / `install` / `uninstall` / `status` /
//! `list_status`）与 `Ide` / `IdeStatus` 两个公开类型。

mod codex_backend;
mod io;
mod json_backends;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use codex_backend::{probe_codex_toml, remove_codex_toml, upsert_codex_toml};
use json_backends::{
    json_command_entry, probe_json_mcp_servers, probe_opencode_json, remove_json_mcp_servers,
    remove_opencode_json, upsert_json_mcp_servers, upsert_opencode_json,
};

/// 受支持的 IDE，对应 4 套差异极大的 MCP 配置格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ide {
    Cursor,
    ClaudeCode,
    Codex,
    OpenCode,
}

impl Ide {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cursor" => Some(Self::Cursor),
            "claude-code" | "claude" | "claude_code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            "opencode" | "open-code" => Some(Self::OpenCode),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
        }
    }

    /// 该 IDE 的「主」MCP 配置文件路径。
    /// 不要带 ~，永远 absolute。
    pub fn primary_config(&self) -> PathBuf {
        let home = dirs::home_dir().expect("INVARIANT: home directory must be resolvable");
        match self {
            Self::Cursor => home.join(".cursor").join("mcp.json"),
            // Claude Code CLI 真正读的是 ~/.claude.json，不是
            // ~/.claude/claude_desktop_config.json（后者是 Claude Desktop App 用的）。
            Self::ClaudeCode => home.join(".claude.json"),
            Self::Codex => home.join(".codex").join("config.toml"),
            Self::OpenCode => home.join(".config").join("opencode").join("opencode.json"),
        }
    }

    pub fn all() -> &'static [Ide] {
        &[Ide::Cursor, Ide::ClaudeCode, Ide::Codex, Ide::OpenCode]
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeStatus {
    pub ide: String,
    pub config_path: String,
    pub config_exists: bool,
    pub installed: bool,
    /// 当前条目里登记的 command 路径（用来检测是否需要覆盖更新）。
    pub command: Option<String>,
}

const SERVER_NAME: &str = "memex";

/// 解析 CLI 的 `memex setup <target>` 入口。保留旧行为：直接 install。
pub fn run(target: &str) -> Result<()> {
    let ide = Ide::parse(target).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown IDE: {}. Supported: cursor, claude-code, codex, opencode",
            target
        )
    })?;
    let memex_bin = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("memex"));
    install(ide, &memex_bin)?;
    println!("\nRestart {} to activate.", ide.as_str());
    Ok(())
}

/// 写入「memex」MCP server 条目。已存在时覆盖更新（让 command 路径跟当前可执行文件走）。
pub fn install(ide: Ide, memex_bin: &Path) -> Result<IdeStatus> {
    let path = ide.primary_config();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    match ide {
        Ide::Cursor | Ide::ClaudeCode => {
            upsert_json_mcp_servers(&path, SERVER_NAME, json_command_entry(memex_bin))?
        }
        Ide::Codex => upsert_codex_toml(&path, SERVER_NAME, memex_bin)?,
        Ide::OpenCode => upsert_opencode_json(&path, SERVER_NAME, memex_bin)?,
    }
    println!("{} MCP configured at {}", ide.as_str(), path.display());
    println!("  command: {} mcp", memex_bin.display());
    status(ide)
}

/// 移除「memex」MCP server 条目。文件不存在或没条目都视为 success（幂等）。
pub fn uninstall(ide: Ide) -> Result<IdeStatus> {
    let path = ide.primary_config();
    if path.exists() {
        match ide {
            Ide::Cursor | Ide::ClaudeCode => remove_json_mcp_servers(&path, SERVER_NAME)?,
            Ide::Codex => remove_codex_toml(&path, SERVER_NAME)?,
            Ide::OpenCode => remove_opencode_json(&path, SERVER_NAME)?,
        }
        println!("{} MCP removed from {}", ide.as_str(), path.display());
    } else {
        println!(
            "{} config not found, nothing to remove ({})",
            ide.as_str(),
            path.display()
        );
    }
    status(ide)
}

pub fn status(ide: Ide) -> Result<IdeStatus> {
    let path = ide.primary_config();
    if !path.exists() {
        return Ok(IdeStatus {
            ide: ide.as_str().to_string(),
            config_path: path.to_string_lossy().to_string(),
            config_exists: false,
            installed: false,
            command: None,
        });
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let (installed, command) = match ide {
        Ide::Cursor | Ide::ClaudeCode => probe_json_mcp_servers(&content, SERVER_NAME),
        Ide::Codex => probe_codex_toml(&content, SERVER_NAME),
        Ide::OpenCode => probe_opencode_json(&content, SERVER_NAME),
    };
    Ok(IdeStatus {
        ide: ide.as_str().to_string(),
        config_path: path.to_string_lossy().to_string(),
        config_exists: true,
        installed,
        command,
    })
}

pub fn list_status() -> Vec<IdeStatus> {
    Ide::all()
        .iter()
        .map(|ide| {
            status(*ide).unwrap_or_else(|_| IdeStatus {
                ide: ide.as_str().to_string(),
                config_path: ide.primary_config().to_string_lossy().to_string(),
                config_exists: false,
                installed: false,
                command: None,
            })
        })
        .collect()
}
