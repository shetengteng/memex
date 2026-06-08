use memex_core::config::MemexConfig;
use memex_core::memex_dir;
use memex_core::storage::db::Db;

fn open_db() -> Result<Db, String> {
    let db_path = memex_dir().join("memex.db");
    Db::open(&db_path).map_err(|e| e.to_string())
}

fn load_config() -> Result<MemexConfig, String> {
    MemexConfig::load(&memex_dir()).map_err(|e| e.to_string())
}

fn save_config(config: &MemexConfig) -> Result<(), String> {
    let content = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    let config_path = memex_dir().join("config.toml");
    std::fs::write(config_path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(key: String) -> Result<Option<String>, String> {
    let config = load_config()?;
    let val = match key.as_str() {
        "llm.ollama_enabled" => Some(config.llm.ollama_enabled.to_string()),
        "llm.ollama_url" => Some(config.llm.ollama_url.clone()),
        "llm.ollama_model" => Some(config.llm.ollama_model.clone()),
        "llm.summarize_interval_ms" => Some(config.llm.summarize_interval_ms.to_string()),
        "privacy.auto_redact" => Some(config.privacy.redaction_enabled.to_string()),
        "privacy.private_from_mcp" => Some(config.privacy.skip_private_sessions.to_string()),
        k if k.starts_with("adapter.") && k.ends_with(".enabled") => {
            let adapter = &k["adapter.".len()..k.len() - ".enabled".len()];
            let enabled = match adapter {
                "claude_code" => config.adapters.claude_code,
                "cursor" => config.adapters.cursor,
                "codex" => config.adapters.codex,
                "opencode" => config.adapters.opencode,
                "aider" => config.adapters.aider,
                "continue_dev" => config.adapters.continue_dev,
                "cline" => config.adapters.cline,
                _ => return Ok(None),
            };
            Some(enabled.to_string())
        }
        _ => {
            let db = open_db()?;
            return db.kv_get(&key).map_err(|e| e.to_string());
        }
    };
    Ok(val)
}

#[tauri::command]
pub async fn set_config(key: String, value: String) -> Result<(), String> {
    let mut config = load_config()?;
    let is_true = value == "true";
    match key.as_str() {
        "llm.ollama_enabled" => {
            config.llm.ollama_enabled = is_true;
            if is_true {
                if config.llm.ollama_url.is_empty() {
                    config.llm.ollama_url = "http://127.0.0.1:11434".to_string();
                }
                if config.llm.ollama_model.is_empty() {
                    config.llm.ollama_model = "qwen2.5:7b".to_string();
                }
            }
        }
        "llm.ollama_url" => config.llm.ollama_url = value.clone(),
        "llm.ollama_model" => config.llm.ollama_model = value.clone(),
        "llm.summarize_interval_ms" => {
            // 容错：把空 / 非法数字归零（=不节流）
            config.llm.summarize_interval_ms = value.parse::<u64>().unwrap_or(0);
        }
        "privacy.auto_redact" => config.privacy.redaction_enabled = is_true,
        "privacy.private_from_mcp" => config.privacy.skip_private_sessions = is_true,
        _ => {
            let db = open_db()?;
            return db.kv_set(&key, &value).map_err(|e| e.to_string());
        }
    }
    save_config(&config)
}

#[tauri::command]
pub async fn toggle_adapter(adapter: String, enabled: bool) -> Result<(), String> {
    let mut config = load_config()?;
    match adapter.as_str() {
        "claude_code" => config.adapters.claude_code = enabled,
        "cursor" => config.adapters.cursor = enabled,
        "codex" => config.adapters.codex = enabled,
        "opencode" => config.adapters.opencode = enabled,
        "aider" => config.adapters.aider = enabled,
        "continue_dev" => config.adapters.continue_dev = enabled,
        "cline" => config.adapters.cline = enabled,
        _ => return Err(format!("unknown adapter: {}", adapter)),
    }
    save_config(&config)
}
