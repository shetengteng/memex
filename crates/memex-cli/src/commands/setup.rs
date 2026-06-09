use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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
        let home = dirs::home_dir().expect("cannot determine home directory");
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
        Ide::Cursor => upsert_json_mcp_servers(&path, SERVER_NAME, json_command_entry(memex_bin))?,
        Ide::ClaudeCode => {
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

// ----- JSON: cursor + claude-code -----

fn json_command_entry(memex_bin: &Path) -> serde_json::Value {
    serde_json::json!({
        "command": memex_bin.to_string_lossy(),
        "args": ["mcp"]
    })
}

fn upsert_json_mcp_servers(path: &Path, server_name: &str, entry: serde_json::Value) -> Result<()> {
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

fn remove_json_mcp_servers(path: &Path, server_name: &str) -> Result<()> {
    let mut config = read_json_or_empty(path)?;
    let Some(obj) = config.as_object_mut() else {
        return Ok(());
    };
    if let Some(servers) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.remove(server_name);
    }
    write_json(path, &config)
}

fn probe_json_mcp_servers(content: &str, server_name: &str) -> (bool, Option<String>) {
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

// ----- JSON: opencode -----

fn upsert_opencode_json(path: &Path, server_name: &str, memex_bin: &Path) -> Result<()> {
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

fn remove_opencode_json(path: &Path, server_name: &str) -> Result<()> {
    let mut config = read_json_or_empty(path)?;
    let Some(obj) = config.as_object_mut() else {
        return Ok(());
    };
    if let Some(mcp) = obj.get_mut("mcp").and_then(|v| v.as_object_mut()) {
        mcp.remove(server_name);
    }
    write_json(path, &config)
}

fn probe_opencode_json(content: &str, server_name: &str) -> (bool, Option<String>) {
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

// ----- TOML: codex -----

fn upsert_codex_toml(path: &Path, server_name: &str, memex_bin: &Path) -> Result<()> {
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

fn remove_codex_toml(path: &Path, server_name: &str) -> Result<()> {
    let mut doc = read_toml_or_empty(path)?;
    if let Some(root) = doc.as_table_mut()
        && let Some(servers) = root.get_mut("mcp_servers").and_then(|v| v.as_table_mut())
    {
        servers.remove(server_name);
    }
    write_toml(path, &doc)
}

fn probe_codex_toml(content: &str, server_name: &str) -> (bool, Option<String>) {
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

// ----- IO helpers -----

fn read_json_or_empty(path: &Path) -> Result<serde_json::Value> {
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

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let output = serde_json::to_string_pretty(value)?;
    fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))
}

fn read_toml_or_empty(path: &Path) -> Result<toml::Value> {
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

fn write_toml(path: &Path, value: &toml::Value) -> Result<()> {
    let output = toml::to_string_pretty(value)?;
    fs::write(path, output).with_context(|| format!("failed to write {}", path.display()))
}
