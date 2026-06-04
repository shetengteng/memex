use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::provider::{LlmProvider, LlmRequest, LlmResponse};

pub struct OllamaProvider {
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct GenerateReq {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    stream: bool,
    options: GenerateOpts,
}

#[derive(Serialize)]
struct GenerateOpts {
    num_predict: usize,
    temperature: f32,
}

#[derive(Deserialize)]
struct GenerateResp {
    response: String,
    model: String,
    #[serde(default)]
    eval_count: usize,
}

#[derive(Deserialize)]
struct TagsResp {
    models: Vec<TagModel>,
}

#[derive(Deserialize)]
struct TagModel {
    name: String,
}

impl OllamaProvider {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
        }
    }

    pub fn from_config(config: &crate::config::LlmConfig) -> Self {
        Self::new(&config.ollama_url, &config.ollama_model)
    }
}

pub fn normalize_ollama_model(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("{}:latest", name)
    }
}

pub fn ollama_model_matches(installed: &str, configured: &str) -> bool {
    normalize_ollama_model(installed) == normalize_ollama_model(configured)
}

/// Base name = part before ':' (e.g. "qwen2.5:7b" -> "qwen2.5").
/// Used to suggest "did you mean ..." when only the tag differs.
pub fn ollama_model_base(name: &str) -> &str {
    name.split(':').next().unwrap_or(name)
}

impl LlmProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url);
        match ureq::get(&url).call() {
            Ok(mut resp) => resp
                .body_mut()
                .read_json::<TagsResp>()
                .map(|t| {
                    t.models
                        .iter()
                        .any(|m| ollama_model_matches(&m.name, &self.model))
                })
                .unwrap_or(false),
            Err(_) => false,
        }
    }

    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse> {
        let url = format!("{}/api/generate", self.base_url);
        let payload = GenerateReq {
            model: self.model.clone(),
            prompt: request.prompt.clone(),
            system: request.system.clone(),
            stream: false,
            options: GenerateOpts {
                num_predict: request.max_tokens,
                temperature: request.temperature,
            },
        };

        let mut resp = ureq::post(&url)
            .send_json(&payload)
            .context("Ollama HTTP request failed")?;

        let parsed: GenerateResp = resp
            .body_mut()
            .read_json()
            .context("failed to parse Ollama response")?;

        Ok(LlmResponse {
            text: parsed.response,
            model: parsed.model,
            tokens_used: parsed.eval_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_name() {
        let p = OllamaProvider::new("http://127.0.0.1:11434", "llama3.2");
        assert_eq!(p.name(), "ollama");
    }

    #[test]
    fn test_from_config() {
        let config = crate::config::LlmConfig {
            ollama_enabled: true,
            ollama_url: "http://localhost:11434".to_string(),
            ollama_model: "qwen2.5".to_string(),
            summary_cooldown_secs: 600,
        };
        let p = OllamaProvider::from_config(&config);
        assert_eq!(p.model, "qwen2.5");
        assert_eq!(p.base_url, "http://localhost:11434");
    }

    #[test]
    fn normalize_adds_latest_when_tag_missing() {
        assert_eq!(normalize_ollama_model("qwen2.5"), "qwen2.5:latest");
        assert_eq!(normalize_ollama_model("qwen2.5:7b"), "qwen2.5:7b");
        assert_eq!(normalize_ollama_model("qwen2.5:latest"), "qwen2.5:latest");
    }

    #[test]
    fn match_accepts_implicit_latest() {
        assert!(ollama_model_matches("qwen2.5:latest", "qwen2.5"));
        assert!(ollama_model_matches("qwen2.5", "qwen2.5:latest"));
        assert!(ollama_model_matches("qwen2.5:7b", "qwen2.5:7b"));
    }

    #[test]
    fn match_rejects_different_tags() {
        assert!(!ollama_model_matches("qwen2.5:latest", "qwen2.5:7b"));
        assert!(!ollama_model_matches("qwen2.5:7b", "qwen2.5:14b"));
        assert!(!ollama_model_matches("llama3.2:latest", "qwen2.5:latest"));
    }

    #[test]
    fn base_strips_tag() {
        assert_eq!(ollama_model_base("qwen2.5:7b"), "qwen2.5");
        assert_eq!(ollama_model_base("qwen2.5"), "qwen2.5");
        assert_eq!(ollama_model_base("registry/llama3.2:latest"), "registry/llama3.2");
    }
}
