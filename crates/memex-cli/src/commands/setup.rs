use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub fn run(target: &str) -> Result<()> {
    let memex_bin = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("memex"));

    match target {
        "cursor" => setup_cursor(&memex_bin),
        "claude-code" | "claude" => setup_claude_code(&memex_bin),
        _ => {
            eprintln!("Unknown target: {}. Supported: cursor, claude-code", target);
            Ok(())
        }
    }
}

fn setup_cursor(memex_bin: &Path) -> Result<()> {
    let home = dirs::home_dir().expect("cannot determine home directory");
    let config_path = home.join(".cursor").join("mcp.json");

    let mcp_entry = serde_json::json!({
        "mcpServers": {
            "memex": {
                "command": memex_bin.to_string_lossy(),
                "args": ["mcp"]
            }
        }
    });

    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let mut config: serde_json::Value =
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

        let servers = config
            .as_object_mut()
            .unwrap()
            .entry("mcpServers")
            .or_insert(serde_json::json!({}));

        servers.as_object_mut().unwrap().insert(
            "memex".to_string(),
            serde_json::json!({
                "command": memex_bin.to_string_lossy(),
                "args": ["mcp"]
            }),
        );

        let output = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, output)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    } else {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let output = serde_json::to_string_pretty(&mcp_entry)?;
        fs::write(&config_path, output)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }

    println!("Cursor MCP configured at {}", config_path.display());
    println!("  command: {} mcp", memex_bin.display());
    println!("\nRestart Cursor to activate.");
    Ok(())
}

fn setup_claude_code(memex_bin: &Path) -> Result<()> {
    let home = dirs::home_dir().expect("cannot determine home directory");
    let config_path = home.join(".claude").join("claude_desktop_config.json");

    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let mut config: serde_json::Value =
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}));

        let servers = config
            .as_object_mut()
            .unwrap()
            .entry("mcpServers")
            .or_insert(serde_json::json!({}));

        servers.as_object_mut().unwrap().insert(
            "memex".to_string(),
            serde_json::json!({
                "command": memex_bin.to_string_lossy(),
                "args": ["mcp"]
            }),
        );

        let output = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, output)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    } else {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let config = serde_json::json!({
            "mcpServers": {
                "memex": {
                    "command": memex_bin.to_string_lossy(),
                    "args": ["mcp"]
                }
            }
        });
        let output = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, output)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }

    println!("Claude Code MCP configured at {}", config_path.display());
    println!("  command: {} mcp", memex_bin.display());
    println!("\nRestart Claude Code to activate.");
    Ok(())
}
