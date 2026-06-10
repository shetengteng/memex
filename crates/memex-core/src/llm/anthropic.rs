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
    /// `"end_turn"` / `"max_tokens"` / `"stop_sequence"` / `"tool_use"`。
    /// `max_tokens` 是 Anthropic 标记"被 max_tokens 截断"的关键信号 ——
    /// 跟 OpenAI 的 `finish_reason="length"` 同义。
    #[serde(default)]
    stop_reason: Option<String>,
}

/// Anthropic content block 有多种 type：
/// - `{"type": "text", "text": "..."}` —— 标准答复
/// - `{"type": "thinking", "thinking": "..."}` —— Claude-3.7-sonnet 等模型
///   开启 extended thinking 时返回的推理过程（与 DeepSeek 的 `reasoning_content`
///   语义一致）。当 max_tokens 太小时，thinking 用光所有 token，text 块根本
///   不出现 —— 跟 DeepSeek-R1 的 reasoning_content 是一样的故障模式。
#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    output_tokens: usize,
}

/// 把 Anthropic 返回 empty content 的报错升级成带可执行诊断的错误信息。
/// 与 OpenAI-compat 的 `empty_content_error` 一一对应：
/// 1. **extended thinking 把 token 用光**：thinking 块有内容 + stop_reason
///    可能是 `max_tokens` —— 报 "请增大 max_tokens（>= 1024）"。
/// 2. **stop_reason == "max_tokens"**：模型回答被 max_tokens 截断 ——
///    报 "请增大 max_tokens 或简化 prompt"。
/// 3. **其它**：服务端真的吐了空 content，保留原始 model 信息。
fn empty_content_error(
    model: &str,
    max_tokens: usize,
    truncated: bool,
    thinking_chars: usize,
) -> anyhow::Error {
    if thinking_chars > 0 {
        anyhow::anyhow!(
            "Anthropic 模型在 extended thinking 阶段消耗了所有 token\
             （thinking 块 {} 字符），text 块为空。\
             max_tokens={} 不足以让模型完成思考后再生成最终答复。\
             请把 LLM Provider 配置里的 max_tokens 调大（建议 >= 1024）。",
            thinking_chars,
            max_tokens,
        )
    } else if truncated {
        anyhow::anyhow!(
            "Anthropic 返回 stop_reason=\"max_tokens\" 但 text 为空（model={}, max_tokens={}）。\
             模型回答被 max_tokens 截断在第一个 token 之前 —— 通常是 prompt 让模型\
             先输出大量空白 / 思考标记。请增大 max_tokens 或简化 prompt。",
            model,
            max_tokens,
        )
    } else {
        anyhow::anyhow!(
            "Anthropic API returned empty content (model={})",
            model,
        )
    }
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
/// 默认走 Haiku 4.5 —— 跟 tars 对齐。摘要/分类这种低复杂度任务用 Haiku
/// 比 Sonnet 便宜约 10 倍且足够好；用户在 Settings → LLM Providers
/// 中创建 anthropic kind 的 provider 时可填写自定义 model 覆盖。
const DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";

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

    pub fn new_direct(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: if model.is_empty() {
                DEFAULT_MODEL.to_string()
            } else {
                model.to_string()
            },
        }
    }
}

impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
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

        // 与 openai_compat::generate 同形态：在 move 前先聚合诊断信号
        // —— stop_reason 是否 max_tokens / 是否有 extended thinking 块写过内容。
        // 这两个信号决定 empty content 时给用户什么样的人话错误。
        let truncated = parsed.stop_reason.as_deref() == Some("max_tokens");
        let thinking_chars: usize = parsed
            .content
            .iter()
            .filter_map(|b| b.thinking.as_ref())
            .map(|t| t.chars().count())
            .sum();

        let text = parsed
            .content
            .into_iter()
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");

        if text.trim().is_empty() {
            return Err(empty_content_error(
                &parsed.model,
                request.max_tokens,
                truncated,
                thinking_chars,
            ));
        }

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

    /// 诊断分支 1（也是最常见的实际故障）：Claude-3.7-sonnet 等模型开了
    /// extended thinking 后，max_tokens 太小让 thinking 把全部 token 用光，
    /// text 块根本没出现。报错必须明确提示 "extended thinking + 增大 max_tokens"
    /// 而不是含糊的 "empty content"。
    #[test]
    fn empty_content_error_for_extended_thinking_runaway() {
        let err = empty_content_error("claude-3-7-sonnet", 8, true, 256);
        let msg = err.to_string();
        assert!(msg.contains("thinking"), "msg should mention thinking: {msg}");
        assert!(
            msg.contains("max_tokens"),
            "msg should suggest increasing max_tokens: {msg}"
        );
        assert!(
            msg.contains("Anthropic"),
            "msg should name the provider family: {msg}"
        );
    }

    /// 诊断分支 2：stop_reason="max_tokens" 但 extended thinking 块没内容
    /// （prompt 把模型引到先输出大段空白 / 思考标记）。
    #[test]
    fn empty_content_error_for_truncated_without_thinking() {
        let err = empty_content_error("claude-haiku-4-5", 4, true, 0);
        let msg = err.to_string();
        assert!(
            msg.contains("max_tokens"),
            "msg should mention max_tokens: {msg}"
        );
        assert!(
            msg.contains("claude-haiku-4-5"),
            "msg should name the model: {msg}"
        );
    }

    /// 诊断分支 3：服务端真的给了空 content 且没有 stop_reason 信号 ——
    /// 这种 case 罕见，保留兜底字符串供运维去对账。
    #[test]
    fn empty_content_error_falls_back_to_generic_when_no_signals() {
        let err = empty_content_error("claude-haiku-4-5", 1024, false, 0);
        let msg = err.to_string();
        assert!(
            msg.contains("Anthropic API returned empty content"),
            "msg should fall back to generic message: {msg}"
        );
        assert!(
            msg.contains("claude-haiku-4-5"),
            "msg should name the model: {msg}"
        );
    }
}
