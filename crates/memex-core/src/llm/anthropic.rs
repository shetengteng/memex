use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::provider::{LlmProvider, LlmRequest, LlmResponse};

pub struct AnthropicProvider {
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<ApiMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ContentBlock>,
    model: String,
    usage: Usage,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    output_tokens: usize,
}

#[derive(Deserialize)]
struct ApiError {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

impl AnthropicProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: DEFAULT_MODEL.to_string(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    pub fn from_env() -> Option<Self> {
        std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .map(|k| Self::new(&k))
    }

    /// 优先从 `~/.memex/credentials.toml` 解析 API key，找不到则回退到
    /// `ANTHROPIC_API_KEY` 环境变量。两边都拿不到可用 key 时返回 `None`。
    pub fn from_credentials_or_env(memex_dir: &std::path::Path) -> Option<Self> {
        let creds = super::credentials::Credentials::load(memex_dir).ok()?;
        let key = creds.resolve_anthropic_key()?;
        let mut provider = Self::new(&key);
        if let Some(model) = creds.resolve_anthropic_model() {
            provider = provider.with_model(&model);
        }
        Some(provider)
    }
}

impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse> {
        let api_req = ApiRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens,
            system: request.system.clone(),
            messages: vec![ApiMessage {
                role: "user".to_string(),
                content: request.prompt.clone(),
            }],
            temperature: request.temperature,
        };

        let mut resp = ureq::post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .send_json(&api_req)
            .context("Anthropic API request failed")?;

        let status = resp.status();
        if status != 200 {
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            let msg = serde_json::from_str::<ApiError>(&body)
                .map(|e| e.error.message)
                .unwrap_or(body);
            anyhow::bail!("Anthropic API error ({}): {}", status, msg);
        }

        let parsed: ApiResponse = resp
            .body_mut()
            .read_json()
            .context("failed to parse Anthropic response")?;

        let text = parsed
            .content
            .into_iter()
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");

        Ok(LlmResponse {
            text,
            model: parsed.model,
            tokens_used: parsed.usage.output_tokens,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_name() {
        let p = AnthropicProvider::new("sk-test-key");
        assert_eq!(p.name(), "anthropic");
    }

    #[test]
    fn test_is_available() {
        let p = AnthropicProvider::new("sk-valid");
        assert!(p.is_available());
        let empty = AnthropicProvider::new("");
        assert!(!empty.is_available());
    }

    #[test]
    fn test_with_model() {
        let p = AnthropicProvider::new("key").with_model("claude-3-haiku-20240307");
        assert_eq!(p.model, "claude-3-haiku-20240307");
    }
}
