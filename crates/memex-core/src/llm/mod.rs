pub mod anthropic;
pub mod credentials;
pub mod ollama;
pub mod provider;
pub mod summarize;

use std::path::Path;

use crate::config::LlmConfig;
use provider::LlmProvider;

pub const CLOUD_NOTICE_KV_KEY: &str = "cloud_fallback_notice_shown";

/// 根据用户的 `LlmConfig` 和 Memex 工作目录（`credentials.toml` 可能在那里），
/// 选出当前最合适的 LLM provider。
///
/// 优先级：
///   1. Ollama（本地）—— 当 `ollama_enabled` 为 true 且 daemon 可达时。
///   2. Anthropic（云端）—— 当 `cloud_fallback` 为 true **且** 有可用的 API key
///      时；key 来源优先级：`~/.memex/credentials.toml`，再回退到
///      `ANTHROPIC_API_KEY` 环境变量。
///   3. `None` —— 调用方把 LLM 能力当作不可用，摘要链路直接跳过，
///      不中断 ingest。
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

/// 描述启用云端兜底时会上传到云端的数据范围。
/// 调用方（CLI、daemon、Tauri）可以在首次调用云端 LLM 之前用这段文字
/// 提示用户。
pub fn cloud_upload_scope() -> String {
    concat!(
        "云端 LLM 兜底已启用（Anthropic）。会发送到云端 API 的数据：\n",
        "  - 已脱敏的 chunk 内容（用于 L1 chunk 摘要）\n",
        "  - 已脱敏的会话消息（用于 L2 会话摘要）\n",
        "  - L2 摘要的标题/主题（用于 L3 项目 / L4 周期摘要）\n",
        "所有内容上传前都会做脱敏（API key、邮箱、IP 等会被替换为 [REDACTED]）。\n",
        "原始来源文件永远不会被上传。关闭方式：memex config set llm.cloud_fallback false",
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
        let provider = select_provider(&cfg, tmp.path()).expect("anthropic 应该被选中");
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
            "cloud_fallback=false 时，即使凭证存在也不应返回 Anthropic"
        );
    }

    #[test]
    fn cloud_upload_scope_contains_key_info() {
        let scope = cloud_upload_scope();
        assert!(scope.contains("脱敏"));
        assert!(scope.contains("cloud_fallback"));
        assert!(scope.contains("Anthropic"));
    }
}
