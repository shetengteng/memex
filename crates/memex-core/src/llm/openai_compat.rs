use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::provider::{LlmProvider, LlmRequest, LlmResponse};

#[cfg(test)]
mod tests;

/// OpenAI Chat Completions 兼容 provider。
/// 支持 DeepSeek、OpenAI、Moonshot、SiliconFlow、Together 等任何说
/// `/v1/chat/completions` 的服务端。
pub struct OpenAiCompatProvider {
    name: String,
    base_url: String,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct ChatCompletionReq {
    model: String,
    messages: Vec<Message>,
    max_tokens: usize,
    temperature: f32,
    stream: bool,
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatCompletionResp {
    choices: Vec<Choice>,
    model: Option<String>,
    #[serde(default)]
    usage: Option<UsageResp>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageResp,
}

#[derive(Deserialize)]
struct MessageResp {
    content: Option<String>,
}

#[derive(Deserialize, Default)]
struct UsageResp {
    #[serde(default)]
    completion_tokens: usize,
}

#[derive(Deserialize)]
struct ModelsResp {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

impl OpenAiCompatProvider {
    pub fn new(name: &str, base_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            name: name.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    fn api_root(&self) -> String {
        if self.base_url.contains("/v1") {
            self.base_url.clone()
        } else {
            format!("{}/v1", self.base_url)
        }
    }

    fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.api_root())
    }

    fn models_url(&self) -> String {
        format!("{}/models", self.api_root())
    }

    pub fn list_models(&self) -> Result<Vec<String>> {
        let mut builder = ureq::get(&self.models_url());
        if !self.api_key.is_empty() {
            builder = builder.header("Authorization", &format!("Bearer {}", self.api_key));
        }
        let mut resp = builder.call().context("models endpoint unreachable")?;
        let parsed: ModelsResp = resp
            .body_mut()
            .read_json()
            .context("failed to parse models response")?;
        let mut ids: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }
}

impl LlmProvider for OpenAiCompatProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse> {
        let mut messages = Vec::new();
        if let Some(sys) = &request.system {
            messages.push(Message {
                role: "system",
                content: sys.clone(),
            });
        }
        messages.push(Message {
            role: "user",
            content: request.prompt.clone(),
        });

        let payload = ChatCompletionReq {
            model: self.model.clone(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            stream: false,
        };

        let mut builder = ureq::post(&self.completions_url());
        if !self.api_key.is_empty() {
            builder = builder.header("Authorization", &format!("Bearer {}", self.api_key));
        }

        let mut resp = builder
            .send_json(&payload)
            .context("OpenAI-compat HTTP request failed")?;

        let status = resp.status();
        if status != 200 {
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            anyhow::bail!("OpenAI-compat API error ({}): {}", status, body);
        }

        let parsed: ChatCompletionResp = resp
            .body_mut()
            .read_json()
            .context("failed to parse OpenAI-compat response")?;

        let text = parsed
            .choices
            .into_iter()
            .filter_map(|c| c.message.content)
            .collect::<Vec<_>>()
            .join("");

        if text.trim().is_empty() {
            anyhow::bail!(
                "OpenAI-compat API returned empty content (model={}, provider={})",
                self.model,
                self.name
            );
        }

        let usage = parsed.usage.unwrap_or_default();

        Ok(LlmResponse {
            text,
            model: parsed.model.unwrap_or_else(|| self.model.clone()),
            tokens_used: usage.completion_tokens,
        })
    }
}
