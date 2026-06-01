use std::fs;
use std::io::{self, BufRead, Write};

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
        "llm.cloud_fallback" => {
            let enabling = parse_bool(value)?;
            if enabling
                && !config.llm.cloud_fallback
                && !cloud_fallback_consent_or_skip(json)?
            {
                if !json {
                    println!("Aborted: llm.cloud_fallback NOT changed.");
                } else {
                    println!(
                        "{}",
                        serde_json::json!({"key": key, "value": value, "status": "aborted_by_user"})
                    );
                }
                return Ok(());
            }
            config.llm.cloud_fallback = enabling;
        }
        "privacy.redaction_enabled" => config.privacy.redaction_enabled = parse_bool(value)?,
        "privacy.skip_private_sessions" => {
            config.privacy.skip_private_sessions = parse_bool(value)?
        }
        "data_dir" => config.data_dir = value.to_string(),
        _ => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({"error": format!("unknown key: {}", key)})
                );
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
        println!(
            "{}",
            serde_json::json!({"key": key, "value": value, "status": "ok"})
        );
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

/// Show the data-egress disclosure for `llm.cloud_fallback = true` and require
/// an interactive yes/no confirmation. JSON / non-TTY callers can opt in
/// non-interactively by setting `MEMEX_CLOUD_FALLBACK_CONSENT=yes`.
fn cloud_fallback_consent_or_skip(json: bool) -> Result<bool> {
    if std::env::var("MEMEX_CLOUD_FALLBACK_CONSENT").as_deref() == Ok("yes") {
        return Ok(true);
    }

    if json {
        anyhow::bail!(
            "enabling llm.cloud_fallback requires interactive consent; \
             set MEMEX_CLOUD_FALLBACK_CONSENT=yes to bypass for scripted use"
        );
    }

    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    writeln!(stderr, "\n⚠️  Cloud fallback is about to be enabled.")?;
    writeln!(stderr, "   What this means:")?;
    writeln!(
        stderr,
        "     • When local Ollama is unavailable, Memex will send REDACTED"
    )?;
    writeln!(
        stderr,
        "       session content (built-in PII rules + your custom redactions.yaml"
    )?;
    writeln!(
        stderr,
        "       applied) to https://api.anthropic.com for L2 session summaries."
    )?;
    writeln!(
        stderr,
        "     • Search results, your raw Markdown, and private sessions never leave the box."
    )?;
    writeln!(
        stderr,
        "     • API key is read from ~/.memex/credentials.toml (chmod 0600) or"
    )?;
    writeln!(stderr, "       the ANTHROPIC_API_KEY environment variable.")?;
    writeln!(stderr, "     • You can revert any time with:")?;
    writeln!(stderr, "         memex config set llm.cloud_fallback false\n")?;
    write!(stderr, "Type `yes` to confirm, anything else to abort: ")?;
    stderr.flush()?;

    let stdin = io::stdin();
    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("yes"))
}
