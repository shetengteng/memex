use memex_core::config::MemexConfig;
use memex_core::memex_dir;
use memex_core::storage::db::Db;

use super::error::{CmdError, CmdResult};

fn open_db() -> CmdResult<Db> {
    let db_path = memex_dir().join("memex.db");
    Ok(Db::open(&db_path)?)
}

fn load_config() -> CmdResult<MemexConfig> {
    Ok(MemexConfig::load(&memex_dir())?)
}

fn save_config(config: &MemexConfig) -> CmdResult<()> {
    let content = toml::to_string_pretty(config).map_err(|e| CmdError::Config(e.to_string()))?;
    let config_path = memex_dir().join("config.toml");
    std::fs::write(config_path, content)?;
    Ok(())
}

#[tauri::command]
pub async fn get_config(key: String) -> CmdResult<Option<String>> {
    let config = load_config()?;
    let val = match key.as_str() {
        "llm.ollama_enabled" => Some(config.llm.ollama_enabled.to_string()),
        "llm.ollama_url" => Some(config.llm.ollama_url.clone()),
        "llm.ollama_model" => Some(config.llm.ollama_model.clone()),
        "llm.summarize_interval_ms" => Some(config.llm.summarize_interval_ms.to_string()),
        // 隐私设置：UI 用 `pref.privacy.*` 前缀（旧约定，跟 `pref.notify.*` 一致），
        // 实际写入的是 config.toml 的 `[privacy]` 段（不是 db kv）。两个 alias 都收。
        // 旧版漏写 alias 时 UI 改了 switch 但不会落地，Bug 直到 2026-06 才修。
        "pref.privacy.auto_redact" | "privacy.auto_redact" => {
            Some(config.privacy.redaction_enabled.to_string())
        }
        "pref.privacy.private_from_mcp" | "privacy.private_from_mcp" => {
            Some(config.privacy.skip_private_sessions.to_string())
        }
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
            return Ok(db.kv_get(&key)?);
        }
    };
    Ok(val)
}

#[tauri::command]
pub async fn set_config(key: String, value: String) -> CmdResult<()> {
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
        // UI 旧约定写 `pref.privacy.*`；既往 fall-through 到 kv 表导致开关失效。
        // 同时支持 toml-native key `privacy.*`（CLI / SKILL.md 文档里使用的形式）。
        "pref.privacy.auto_redact" | "privacy.auto_redact" => {
            config.privacy.redaction_enabled = is_true;
        }
        "pref.privacy.private_from_mcp" | "privacy.private_from_mcp" => {
            config.privacy.skip_private_sessions = is_true;
        }
        _ => {
            let db = open_db()?;
            db.kv_set(&key, &value)?;
            return Ok(());
        }
    }
    save_config(&config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn with_temp_memex<F: FnOnce()>(f: F) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::var("MEMEX_HOME").ok();
        // SAFETY: 由 #[serial(memex_home)] 串行化
        unsafe { std::env::set_var("MEMEX_HOME", tmp.path()) };
        f();
        match prev {
            Some(v) => unsafe { std::env::set_var("MEMEX_HOME", v) },
            None => unsafe { std::env::remove_var("MEMEX_HOME") },
        }
    }

    fn block<F: std::future::Future>(f: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f)
    }

    /// UI 用 `pref.privacy.auto_redact` 这个 key 写 switch；之前版本
    /// 因 commands/config.rs match arm 是 `privacy.auto_redact`（没 prefix），
    /// 触发 fall-through 到 db kv 表，TOML 里的 `redaction_enabled` 永远不变。
    /// 这个测试钉住"UI key 真的能落到 toml"。
    #[test]
    #[serial(memex_home)]
    fn ui_pref_privacy_auto_redact_persists_to_toml() {
        with_temp_memex(|| {
            block(set_config("pref.privacy.auto_redact".into(), "false".into()))
                .expect("set_config ok");
            let cfg = MemexConfig::load(&memex_dir()).expect("load");
            assert!(!cfg.privacy.redaction_enabled, "toml 应被翻成 false");

            let val = block(get_config("pref.privacy.auto_redact".into())).expect("get ok");
            assert_eq!(val.as_deref(), Some("false"));

            // toml-native key 也应读得到同一个值
            let val2 = block(get_config("privacy.auto_redact".into())).expect("get ok");
            assert_eq!(val2.as_deref(), Some("false"));
        });
    }

    #[test]
    #[serial(memex_home)]
    fn ui_pref_privacy_private_from_mcp_persists_to_toml() {
        with_temp_memex(|| {
            block(set_config(
                "pref.privacy.private_from_mcp".into(),
                "true".into(),
            ))
            .expect("ok");
            let cfg = MemexConfig::load(&memex_dir()).expect("load");
            assert!(cfg.privacy.skip_private_sessions, "toml 写入 true");

            let val = block(get_config("pref.privacy.private_from_mcp".into())).expect("ok");
            assert_eq!(val.as_deref(), Some("true"));
        });
    }
}

#[tauri::command]
pub async fn toggle_adapter(adapter: String, enabled: bool) -> CmdResult<()> {
    let mut config = load_config()?;
    match adapter.as_str() {
        "claude_code" => config.adapters.claude_code = enabled,
        "cursor" => config.adapters.cursor = enabled,
        "codex" => config.adapters.codex = enabled,
        "opencode" => config.adapters.opencode = enabled,
        "aider" => config.adapters.aider = enabled,
        "continue_dev" => config.adapters.continue_dev = enabled,
        "cline" => config.adapters.cline = enabled,
        _ => {
            return Err(CmdError::Validation(format!(
                "unknown adapter: {}",
                adapter
            )));
        }
    }
    save_config(&config)
}
