pub mod anthropic;
pub mod credentials;
pub mod ollama;
pub mod provider;
pub mod summarize;

use std::path::Path;

use crate::config::LlmConfig;
use provider::LlmProvider;

pub const CLOUD_NOTICE_KV_KEY: &str = "cloud_fallback_notice_shown";

/// Select the best available LLM provider, given the user's `LlmConfig` and
/// the on-disk Memex working directory (where `credentials.toml` may live).
///
/// Priority:
///   1. Ollama (local) when `ollama_enabled` is true and the daemon is reachable.
///   2. Anthropic (cloud) when `cloud_fallback` is true **and** an API key is
///      available — first via `~/.memex/credentials.toml`, then via the
///      `ANTHROPIC_API_KEY` environment variable.
///   3. `None` — caller treats LLM features as unavailable and skips the
///      summary path without aborting ingest.
pub fn select_provider(config: &LlmConfig, memex_dir: &Path) -> Option<Box<dyn LlmProvider>> {
    if config.ollama_enabled {
        let ollama = ollama::OllamaProvider::from_config(config);
        if ollama.is_available() {
            return Some(Box::new(ollama));
        }
    }

    if config.cloud_fallback {
        if let Some(provider) = anthropic::AnthropicProvider::from_credentials_or_env(memex_dir) {
            if provider.is_available() {
                tracing::info!("{}", cloud_upload_scope());
                return Some(Box::new(provider));
            }
        }
    }

    None
}

/// Describes what data scope is uploaded when cloud fallback is active.
/// Callers (CLI, daemon, Tauri) can use this to display a notice before
/// the first cloud LLM call.
pub fn cloud_upload_scope() -> String {
    concat!(
        "Cloud LLM fallback active (Anthropic). Data sent to cloud API:\n",
        "  - Redacted chunk content (for L1 chunk summaries)\n",
        "  - Redacted session messages (for L2 session summaries)\n",
        "  - L2 summary titles/topics (for L3 project / L4 periodic summaries)\n",
        "All content is redacted before upload (API keys, emails, IPs, etc. replaced with [REDACTED]).\n",
        "Raw source files are never uploaded. Disable with: memex config set llm.cloud_fallback false",
    ).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::credentials::{AnthropicCredentials, Credentials};
    use tempfile::TempDir;

    fn disabled_config() -> LlmConfig {
        LlmConfig {
            ollama_enabled: false,
            ollama_url: "http://127.0.0.1:0".into(),
            ollama_model: "test".into(),
            cloud_fallback: false,
        }
    }

    #[test]
    fn returns_none_when_all_paths_disabled() {
        let tmp = TempDir::new().unwrap();
        let provider = select_provider(&disabled_config(), tmp.path());
        assert!(provider.is_none());
    }

    #[test]
    fn picks_anthropic_when_credentials_file_present() {
        let tmp = TempDir::new().unwrap();
        Credentials {
            anthropic: Some(AnthropicCredentials {
                api_key: "sk-ant-test".into(),
                model: None,
            }),
        }
        .save(tmp.path())
        .unwrap();
        let mut cfg = disabled_config();
        cfg.cloud_fallback = true;
        let provider = select_provider(&cfg, tmp.path()).expect("anthropic should be selected");
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn skips_anthropic_when_cloud_fallback_disabled() {
        let tmp = TempDir::new().unwrap();
        Credentials {
            anthropic: Some(AnthropicCredentials {
                api_key: "sk-ant-test".into(),
                model: None,
            }),
        }
        .save(tmp.path())
        .unwrap();
        let provider = select_provider(&disabled_config(), tmp.path());
        assert!(
            provider.is_none(),
            "cloud_fallback=false should not return Anthropic even if creds exist"
        );
    }

    #[test]
    fn cloud_upload_scope_contains_key_info() {
        let scope = cloud_upload_scope();
        assert!(scope.contains("Redacted"));
        assert!(scope.contains("cloud_fallback"));
        assert!(scope.contains("Anthropic"));
    }
}
