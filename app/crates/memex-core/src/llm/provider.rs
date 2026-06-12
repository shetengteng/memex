use anyhow::Result;

pub struct LlmRequest {
    pub system: Option<String>,
    pub prompt: String,
    pub max_tokens: usize,
    pub temperature: f32,
}

pub struct LlmResponse {
    pub text: String,
    pub model: String,
    pub tokens_used: usize,
}

pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn generate(&self, request: &LlmRequest) -> Result<LlmResponse>;
    /// 当前 provider 实际请求的模型字符串（如 `qwen2.5:7b` / `gpt-4o-mini` /
    /// `deepseek-v4-flash` / `claude-haiku-4-5-20251001`）。
    ///
    /// UI 用它在 sidebar / dashboard 上把"当前 LLM"展示成
    /// `Provider · model` 而不是只展示 provider name —— 用户能直接看到自己
    /// 配置的是哪个模型在工作。默认实现返回空字符串，是给测试 / mock provider
    /// 留的退路；正经的 provider（Ollama / OpenAI-compat / Anthropic）必须
    /// override 这个方法。
    fn model(&self) -> &str {
        ""
    }
}

impl Default for LlmRequest {
    fn default() -> Self {
        Self {
            system: None,
            prompt: String::new(),
            max_tokens: 2048,
            temperature: 0.3,
        }
    }
}

impl LlmRequest {
    pub fn with_prompt(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            ..Default::default()
        }
    }

    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}
