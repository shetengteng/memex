use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::provider::{LlmProvider, LlmRequest, LlmResponse};

/// 把 "API returned empty content" 的报错升级成带可执行诊断的错误信息。
///
/// 三种典型场景：
/// 1. **reasoning model + 小 max_tokens**：reasoning_content 已写入，
///    但 token 配额耗尽前没轮到 content 阶段 —— 报 `enlarge max_tokens`。
/// 2. **finish_reason == "length"**：模型回答被 max_tokens 截断（普通模型
///    也可能发生在 prompt 引导出超长答案时）—— 同样报 `enlarge max_tokens`。
/// 3. **其它**：服务端真的吐了空 content（罕见），保留原始 model/provider
///    信息让用户去对账。
fn empty_content_error(
    model: &str,
    provider_name: &str,
    max_tokens: usize,
    any_truncated: bool,
    reasoning_chars: usize,
) -> anyhow::Error {
    if reasoning_chars > 0 {
        anyhow::anyhow!(
            "{} 模型在 reasoning 阶段消耗了所有 token（reasoning_content {} 字符），\
             content 字段为空。\
             检测到 reasoning_content —— 这是 DeepSeek-R1 / V4 等 reasoning model \
             的特征，max_tokens={} 不足以让模型完成推理后再生成最终答案。\
             请把 LLM Provider 配置里的 max_tokens 调大（建议 >= 1024）。",
            provider_name,
            reasoning_chars,
            max_tokens,
        )
    } else if any_truncated {
        anyhow::anyhow!(
            "{} 返回 finish_reason=\"length\" 但 content 为空（model={}, max_tokens={}）。\
             模型回答被 max_tokens 截断在第一个 token 之前 —— 通常是 prompt 让模型\
             先输出大量空白 / 思考标记。请增大 max_tokens 或简化 prompt。",
            provider_name,
            model,
            max_tokens,
        )
    } else {
        anyhow::anyhow!(
            "OpenAI-compat API returned empty content (model={}, provider={})",
            model,
            provider_name
        )
    }
}

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
    /// `"stop"` / `"length"` / `"content_filter"` / `"tool_calls"` 等。
    /// `length` 表示答案被 max_tokens 截断 —— 对 reasoning model 这是
    /// "思考阶段已经用光 token 配额，没有 token 给最终回答" 的关键信号。
    #[serde(default)]
    finish_reason: Option<String>,
}

/// DeepSeek-R1 / V4 等 reasoning model 把推理过程放在 `reasoning_content`,
/// 最终答案放在 `content`。当 max_tokens 太小时，content 会是空字符串，
/// 但 reasoning_content 已经写入；据此可以给用户精确诊断 "max_tokens
/// 太小，请调大配置"。
#[derive(Deserialize)]
struct MessageResp {
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
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

        // 在 move 出 choices 前先聚合诊断信号：finish_reason 是否 length /
        // 是否 reasoning model 写了 reasoning_content。empty content 时这些
        // 决定我们给用户什么样的人话错误。
        let any_truncated = parsed
            .choices
            .iter()
            .any(|c| c.finish_reason.as_deref() == Some("length"));
        let reasoning_total_chars: usize = parsed
            .choices
            .iter()
            .filter_map(|c| c.message.reasoning_content.as_ref())
            .map(|r| r.chars().count())
            .sum();

        let text = parsed
            .choices
            .into_iter()
            .filter_map(|c| c.message.content)
            .collect::<Vec<_>>()
            .join("");

        if text.trim().is_empty() {
            return Err(empty_content_error(
                &self.model,
                &self.name,
                request.max_tokens,
                any_truncated,
                reasoning_total_chars,
            ));
        }

        let usage = parsed.usage.unwrap_or_default();

        Ok(LlmResponse {
            text,
            model: parsed.model.unwrap_or_else(|| self.model.clone()),
            tokens_used: usage.completion_tokens,
        })
    }
}
