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
        "adapters.aider" => config.adapters.aider = parse_bool(value)?,
        "adapters.continue_dev" => config.adapters.continue_dev = parse_bool(value)?,
        "adapters.cline" => config.adapters.cline = parse_bool(value)?,
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
        _ => anyhow::bail!("无效的布尔值：{}", s),
    }
}

/// 在开启 `llm.cloud_fallback = true` 之前显示数据出境说明，
/// 并要求用户交互式输入 yes/no 确认。
/// JSON / 非 TTY 调用方可以通过设置 `MEMEX_CLOUD_FALLBACK_CONSENT=yes`
/// 来非交互地表示同意。
fn cloud_fallback_consent_or_skip(json: bool) -> Result<bool> {
    if std::env::var("MEMEX_CLOUD_FALLBACK_CONSENT").as_deref() == Ok("yes") {
        return Ok(true);
    }

    if json {
        anyhow::bail!(
            "开启 llm.cloud_fallback 需要交互式确认；脚本环境可设置 \
             MEMEX_CLOUD_FALLBACK_CONSENT=yes 跳过提示"
        );
    }

    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    writeln!(stderr, "\n⚠️  即将开启云端兜底（cloud fallback）。")?;
    writeln!(stderr, "   含义如下：")?;
    writeln!(
        stderr,
        "     • 本地 Ollama 不可用时，Memex 会把【已脱敏】的会话内容"
    )?;
    writeln!(
        stderr,
        "       （内置 PII 规则 + 你自己的 redactions.yaml 都会先应用一遍）"
    )?;
    writeln!(
        stderr,
        "       发送到 https://api.anthropic.com 用于生成 L2 会话摘要。"
    )?;
    writeln!(
        stderr,
        "     • 搜索结果、原始 Markdown、private 会话永远不会离开本机。"
    )?;
    writeln!(
        stderr,
        "     • API key 从 ~/.memex/credentials.toml（chmod 0600）或"
    )?;
    writeln!(stderr, "       ANTHROPIC_API_KEY 环境变量读取。")?;
    writeln!(stderr, "     • 随时可以关闭：")?;
    writeln!(stderr, "         memex config set llm.cloud_fallback false\n")?;
    write!(stderr, "输入 `yes` 表示确认，其它任何输入都会取消：")?;
    stderr.flush()?;

    let stdin = io::stdin();
    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("yes"))
}
