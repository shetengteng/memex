use std::time::Instant;

use memex_core::config::MemexConfig;
use memex_core::llm::ollama::{OllamaProvider, ollama_model_base};
use memex_core::llm::provider::{LlmProvider, LlmRequest};
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
                    (
                        "Ollama is running but has no models installed".into(),
                        Some(names),
                    )
                } else {
                    let configured = &config.llm.ollama_model;
                    let configured_base = ollama_model_base(configured);
                    let suggestion = names
                        .iter()
                        .find(|n| ollama_model_base(n) == configured_base)
                        .cloned();
                    let msg = match suggestion {
                        Some(s) => format!(
                            "Model '{}' not found. Did you mean '{}'? Available: {}",
                            configured,
                            s,
                            names.join(", ")
                        ),
                        None => format!(
                            "Model '{}' not found. Available: {}",
                            configured,
                            names.join(", ")
                        ),
                    };
                    (msg, Some(names))
                }
            }
            Err(e) => (
                format!("Cannot reach Ollama at {}: {}", config.llm.ollama_url, e),
                None,
            ),
        };
        return Ok(LlmTestResult {
            ok: false,
            latency_ms: elapsed,
            error: Some(err),
            response_text: None,
            models_available: models,
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
            })
        }
    }
}
