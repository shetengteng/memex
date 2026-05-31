use std::fs;

use anyhow::{Context, Result};
use memex_core::config::MemexConfig;
use memex_core::memex_dir;

pub fn show(json: bool) -> Result<()> {
    let config = MemexConfig::load(&memex_dir())?;

    if json {
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        println!("{}", toml::to_string_pretty(&config)?);
    }

    Ok(())
}

pub fn set(key: &str, value: &str, json: bool) -> Result<()> {
    let memex = memex_dir();
    let config_path = memex.join("config.toml");
    let mut config = MemexConfig::load(&memex)?;

    match key {
        "adapters.claude_code" => config.adapters.claude_code = parse_bool(value)?,
        "adapters.cursor" => config.adapters.cursor = parse_bool(value)?,
        "adapters.codex" => config.adapters.codex = parse_bool(value)?,
        "adapters.opencode" => config.adapters.opencode = parse_bool(value)?,
        "llm.ollama_enabled" => config.llm.ollama_enabled = parse_bool(value)?,
        "llm.ollama_url" => config.llm.ollama_url = value.to_string(),
        "llm.ollama_model" => config.llm.ollama_model = value.to_string(),
        "llm.cloud_fallback" => config.llm.cloud_fallback = parse_bool(value)?,
        "privacy.redaction_enabled" => config.privacy.redaction_enabled = parse_bool(value)?,
        "privacy.skip_private_sessions" => config.privacy.skip_private_sessions = parse_bool(value)?,
        "data_dir" => config.data_dir = value.to_string(),
        _ => {
            if json {
                println!("{}", serde_json::json!({"error": format!("unknown key: {}", key)}));
            } else {
                eprintln!("Unknown config key: {}", key);
            }
            return Ok(());
        }
    }

    let content = toml::to_string_pretty(&config)?;
    fs::write(&config_path, &content)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    if json {
        println!("{}", serde_json::json!({"key": key, "value": value, "status": "ok"}));
    } else {
        println!("Set {} = {}", key, value);
    }

    Ok(())
}

fn parse_bool(s: &str) -> Result<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => anyhow::bail!("invalid boolean value: {}", s),
    }
}
