pub mod anthropic;
pub mod ollama;
pub mod openai_compat;
pub mod provider;
pub mod summarize;

use std::path::Path;

use crate::config::LlmConfig;
use crate::storage::db::{Db, LlmProviderRow};
use provider::LlmProvider;

/// 从 DB 中已注册的 provider 列表构建 LLM client。
/// is_default 排在最前面，然后按 name 排序。
/// 逐个尝试，第一个 `is_available()` 的胜出。
pub fn select_provider_from_db(db: &Db) -> Option<Box<dyn LlmProvider>> {
    let rows = db.provider_list().ok()?;
    for row in rows.iter().filter(|r| r.enabled) {
        if let Some(p) = build_provider_from_row(row) {
            if p.is_available() {
                return Some(p);
            }
        }
    }
    None
}

/// 从一行 DB 记录构建具体的 LlmProvider 实例。
/// 支持的 kind：`ollama` / `openai_compat` / `anthropic`。
pub fn build_provider_from_row(row: &LlmProviderRow) -> Option<Box<dyn LlmProvider>> {
    match row.kind.as_str() {
        "ollama" => {
            let p = ollama::OllamaProvider::new(&row.base_url, &row.model);
            Some(Box::new(p))
        }
        "openai_compat" => {
            let p = openai_compat::OpenAiCompatProvider::new(
                &row.name,
                &row.base_url,
                &row.api_key,
                &row.model,
            );
            Some(Box::new(p))
        }
        "anthropic" => {
            let p = anthropic::AnthropicProvider::new_direct(&row.api_key, &row.model);
            Some(Box::new(p))
        }
        _ => None,
    }
}

/// 统一选择入口：优先从 DB 取已注册的 provider，DB 为空时回退到
/// 老的 `LlmConfig.ollama_*` 配置（仅支持 Ollama 快捷入口）。
pub fn select_provider_unified(
    db: &Db,
    config: &LlmConfig,
    memex_dir: &Path,
) -> Option<Box<dyn LlmProvider>> {
    if let Ok(rows) = db.provider_list() {
        if !rows.is_empty() {
            return select_provider_from_db(db);
        }
    }
    select_provider(config, memex_dir)
}

/// 仅依据 `LlmConfig` 中的 Ollama 老快捷入口选择 provider。
/// DeepSeek / Anthropic / 其他云端 provider 一律走 DB providers，
/// 不再从 config.toml 或 credentials.toml 中读取。
pub fn select_provider(config: &LlmConfig, _memex_dir: &Path) -> Option<Box<dyn LlmProvider>> {
    if config.ollama_enabled {
        let ollama = ollama::OllamaProvider::from_config(config);
        if ollama.is_available() {
            return Some(Box::new(ollama));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn disabled_config() -> LlmConfig {
        LlmConfig {
            ollama_enabled: false,
            ollama_url: "http://127.0.0.1:0".into(),
            ollama_model: "test".into(),
            summary_cooldown_secs: 600,
        }
    }

    #[test]
    fn returns_none_when_all_paths_disabled() {
        let tmp = TempDir::new().unwrap();
        let provider = select_provider(&disabled_config(), tmp.path());
        assert!(provider.is_none());
    }

    #[test]
    fn returns_none_when_ollama_unreachable() {
        let tmp = TempDir::new().unwrap();
        let cfg = LlmConfig {
            ollama_enabled: true,
            ollama_url: "http://127.0.0.1:0".into(),
            ollama_model: "test".into(),
            summary_cooldown_secs: 600,
        };
        let provider = select_provider(&cfg, tmp.path());
        assert!(
            provider.is_none(),
            "ollama_url 指向不可达端口时，不应该返回 provider"
        );
    }
}
