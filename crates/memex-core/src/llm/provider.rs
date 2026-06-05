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
