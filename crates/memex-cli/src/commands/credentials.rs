//! `memex credentials` — manage `~/.memex/credentials.toml`.
//!
//! Surface area:
//!     * `set anthropic <key> [--model M]`  — write key (chmod 0600 on Unix).
//!     * `show`                              — print known providers without leaking the key.
//!     * `clear anthropic`                   — wipe the anthropic block.

use anyhow::Result;
use memex_core::llm::credentials::{AnthropicCredentials, Credentials};
use memex_core::memex_dir;

pub fn set_anthropic(api_key: &str, model: Option<String>, json: bool) -> Result<()> {
    let memex = memex_dir();
    let mut creds = Credentials::load(&memex).unwrap_or_default();

    if api_key.trim().is_empty() {
        let msg = "anthropic api_key cannot be empty (got whitespace or empty string)";
        emit_error(json, msg);
        return Ok(());
    }

    creds.anthropic = Some(AnthropicCredentials {
        api_key: api_key.trim().to_string(),
        model: model.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()),
    });
    creds.save(&memex)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "provider": "anthropic",
                "path": super::credentials_path_string(&memex),
                "model": creds.anthropic.as_ref().and_then(|a| a.model.clone()),
            })
        );
    } else {
        println!(
            "Anthropic credentials saved to {} (chmod 0600).",
            super::credentials_path_string(&memex)
        );
        if let Some(m) = creds.anthropic.as_ref().and_then(|a| a.model.as_deref()) {
            println!("  model: {}", m);
        } else {
            println!("  model: (default — claude-sonnet-4-20250514)");
        }
        println!("\nEnable cloud fallback with:\n  memex config set llm.cloud_fallback true");
    }
    Ok(())
}

pub fn show(json: bool) -> Result<()> {
    let memex = memex_dir();
    let creds = Credentials::load(&memex).unwrap_or_default();

    let anthropic_state = match &creds.anthropic {
        Some(a) if !a.api_key.trim().is_empty() => "set",
        _ => match std::env::var("ANTHROPIC_API_KEY") {
            Ok(v) if !v.trim().is_empty() => "set (env)",
            _ => "missing",
        },
    };

    let model = creds
        .anthropic
        .as_ref()
        .and_then(|a| a.model.clone())
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .unwrap_or_else(|| "claude-sonnet-4-20250514 (default)".into());

    if json {
        println!(
            "{}",
            serde_json::json!({
                "path": super::credentials_path_string(&memex),
                "anthropic": {
                    "api_key_status": anthropic_state,
                    "model": model,
                }
            })
        );
    } else {
        println!("Credentials file: {}", super::credentials_path_string(&memex));
        println!("\nAnthropic:");
        println!("  api_key: {}", anthropic_state);
        println!("  model:   {}", model);
    }
    Ok(())
}

pub fn clear_anthropic(json: bool) -> Result<()> {
    let memex = memex_dir();
    let mut creds = Credentials::load(&memex).unwrap_or_default();
    let was_set = creds.anthropic.is_some();
    creds.anthropic = None;
    creds.save(&memex)?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "ok": true,
                "provider": "anthropic",
                "was_set": was_set,
            })
        );
    } else if was_set {
        println!("Anthropic credentials cleared from credentials.toml.");
    } else {
        println!("No Anthropic credentials were stored.");
    }
    Ok(())
}

fn emit_error(json: bool, msg: &str) {
    if json {
        println!("{}", serde_json::json!({ "error": msg }));
    } else {
        eprintln!("{}", msg);
    }
}
