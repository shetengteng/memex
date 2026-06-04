use std::time::Instant;

use memex_core::config::MemexConfig;
use memex_core::llm::anthropic::AnthropicProvider;
use memex_core::llm::credentials::{AnthropicCredentials, Credentials, DeepSeekCredentials};
use memex_core::llm::ollama::OllamaProvider;
use memex_core::llm::provider::{LlmProvider, LlmRequest};
use memex_core::llm::{self};
use memex_core::memex_dir;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LlmTestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
    pub response_text: Option<String>,
    /// Ollama only: models found on the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models_available: Option<Vec<String>>,
    /// Anthropic only: where the key was loaded from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_source: Option<String>,
}

fn micro_request() -> LlmRequest {
    LlmRequest {
        prompt: "Reply with exactly one word: OK".to_string(),
        system: None,
        max_tokens: 8,
        temperature: 0.0,
    }
}

#[tauri::command]
pub async fn llm_test_ollama() -> Result<LlmTestResult, String> {
    let config = MemexConfig::load(&memex_dir()).map_err(|e| e.to_string())?;
    let provider = OllamaProvider::from_config(&config.llm);
    let start = Instant::now();

    if !provider.is_available() {
        let elapsed = start.elapsed().as_millis() as u64;
        let tags_url = format!("{}/api/tags", config.llm.ollama_url);
        let (err, models) = match ureq::get(&tags_url).call() {
            Ok(mut resp) => {
                let names: Vec<String> = resp
                    .body_mut()
                    .read_json::<serde_json::Value>()
                    .ok()
                    .and_then(|v| v["models"].as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|m| m["name"].as_str().map(String::from))
                    .collect();
                if names.is_empty() {
                    ("Ollama is running but has no models installed".into(), Some(names))
                } else {
                    (
                        format!(
                            "Model '{}' not found. Available: {}",
                            config.llm.ollama_model,
                            names.join(", ")
                        ),
                        Some(names),
                    )
                }
            }
            Err(e) => (format!("Cannot reach Ollama at {}: {}", config.llm.ollama_url, e), None),
        };
        return Ok(LlmTestResult {
            ok: false,
            latency_ms: elapsed,
            error: Some(err),
            response_text: None,
            models_available: models,
            key_source: None,
        });
    }

    match provider.generate(&micro_request()) {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(LlmTestResult {
                ok: true,
                latency_ms: elapsed,
                error: None,
                response_text: Some(resp.text),
                models_available: None,
                key_source: None,
            })
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(LlmTestResult {
                ok: false,
                latency_ms: elapsed,
                error: Some(e.to_string()),
                response_text: None,
                models_available: None,
                key_source: None,
            })
        }
    }
}

#[tauri::command]
pub async fn llm_test_anthropic() -> Result<LlmTestResult, String> {
    let dir = memex_dir();
    let start = Instant::now();

    let creds = Credentials::load(&dir).unwrap_or_default();
    let key_source = if creds
        .anthropic
        .as_ref()
        .map(|a| !a.api_key.trim().is_empty())
        .unwrap_or(false)
    {
        "credentials.toml"
    } else if std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .is_some()
    {
        "ANTHROPIC_API_KEY env"
    } else {
        let elapsed = start.elapsed().as_millis() as u64;
        return Ok(LlmTestResult {
            ok: false,
            latency_ms: elapsed,
            error: Some(
                "No API key found. Set it via Settings or run: memex credentials set anthropic --key sk-ant-..."
                    .into(),
            ),
            response_text: None,
            models_available: None,
            key_source: Some("none".into()),
        });
    };

    let provider = match AnthropicProvider::from_credentials_or_env(&dir) {
        Some(p) => p,
        None => {
            let elapsed = start.elapsed().as_millis() as u64;
            return Ok(LlmTestResult {
                ok: false,
                latency_ms: elapsed,
                error: Some("Failed to initialize Anthropic provider".into()),
                response_text: None,
                models_available: None,
                key_source: Some(key_source.into()),
            });
        }
    };

    match provider.generate(&micro_request()) {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(LlmTestResult {
                ok: true,
                latency_ms: elapsed,
                error: None,
                response_text: Some(resp.text),
                models_available: None,
                key_source: Some(key_source.into()),
            })
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(LlmTestResult {
                ok: false,
                latency_ms: elapsed,
                error: Some(e.to_string()),
                response_text: None,
                models_available: None,
                key_source: Some(key_source.into()),
            })
        }
    }
}

#[tauri::command]
pub async fn save_anthropic_key(api_key: String) -> Result<(), String> {
    let dir = memex_dir();
    let mut creds = Credentials::load(&dir).unwrap_or_default();
    if let Some(ref mut a) = creds.anthropic {
        a.api_key = api_key;
    } else {
        creds.anthropic = Some(AnthropicCredentials {
            api_key,
            model: None,
        });
    }
    creds.save(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_test_deepseek() -> Result<LlmTestResult, String> {
    let dir = memex_dir();
    let config = MemexConfig::load(&dir).map_err(|e| e.to_string())?;
    let start = Instant::now();

    let creds = Credentials::load(&dir).unwrap_or_default();
    let key_source = if creds
        .deepseek
        .as_ref()
        .map(|d| !d.api_key.trim().is_empty())
        .unwrap_or(false)
    {
        "credentials.toml"
    } else if std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .is_some()
    {
        "DEEPSEEK_API_KEY env"
    } else {
        let elapsed = start.elapsed().as_millis() as u64;
        return Ok(LlmTestResult {
            ok: false,
            latency_ms: elapsed,
            error: Some(
                "No API key found. Set it via Settings or run: memex credentials set deepseek --key sk-..."
                    .into(),
            ),
            response_text: None,
            models_available: None,
            key_source: Some("none".into()),
        });
    };

    let provider = match build_deepseek_provider(&config.llm, &dir) {
        Some(p) => p,
        None => {
            let elapsed = start.elapsed().as_millis() as u64;
            return Ok(LlmTestResult {
                ok: false,
                latency_ms: elapsed,
                error: Some("Failed to initialize DeepSeek provider".into()),
                response_text: None,
                models_available: None,
                key_source: Some(key_source.into()),
            });
        }
    };

    match provider.generate(&micro_request()) {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(LlmTestResult {
                ok: true,
                latency_ms: elapsed,
                error: None,
                response_text: Some(resp.text),
                models_available: None,
                key_source: Some(key_source.into()),
            })
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(LlmTestResult {
                ok: false,
                latency_ms: elapsed,
                error: Some(e.to_string()),
                response_text: None,
                models_available: None,
                key_source: Some(key_source.into()),
            })
        }
    }
}

fn build_deepseek_provider(
    config: &memex_core::config::LlmConfig,
    memex_dir: &std::path::Path,
) -> Option<memex_core::llm::openai_compat::OpenAiCompatProvider> {
    llm::build_deepseek_provider(config, memex_dir)
}

#[tauri::command]
pub async fn save_deepseek_key(api_key: String) -> Result<(), String> {
    let dir = memex_dir();
    let mut creds = Credentials::load(&dir).unwrap_or_default();
    if let Some(ref mut d) = creds.deepseek {
        d.api_key = api_key;
    } else {
        creds.deepseek = Some(DeepSeekCredentials {
            api_key,
            model: None,
        });
    }
    creds.save(&dir).map_err(|e| e.to_string())
}
