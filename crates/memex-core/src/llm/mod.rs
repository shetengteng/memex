pub mod anthropic;
pub mod credentials;
pub mod ollama;
pub mod openai_compat;
pub mod provider;
pub mod summarize;

use std::path::Path;

use crate::config::LlmConfig;
use crate::storage::db::{Db, LlmProviderRow};
use provider::LlmProvider;

pub const CLOUD_NOTICE_KV_KEY: &str = "cloud_fallback_notice_shown";

/// 从 DB 中已注册的 provider 列表构建 LLM client。
/// is_default 排在最前面，然后按 name 排序。
/// 逐个尝试，第一个 `is_available()` 的胜出。
pub fn select_provider_from_db(db: &Db) -> Option<Box<dyn LlmProvider>> {
    let rows = db.provider_list().ok()?;
    for row in rows.iter().filter(|r| r.enabled) {
        if let Some(p) = build_provider_from_row(row) {
            if p.is_available() {
                return Some(p);
            }
        }
    }
    None
}

/// 从一行 DB 记录构建具体的 LlmProvider 实例。
pub fn build_provider_from_row(row: &LlmProviderRow) -> Option<Box<dyn LlmProvider>> {
    match row.kind.as_str() {
        "ollama" => {
            let p = ollama::OllamaProvider::new(&row.base_url, &row.model);
            Some(Box::new(p))
        }
        "openai_compat" => {
            let p = openai_compat::OpenAiCompatProvider::new(
                &row.name,
                &row.base_url,
                &row.api_key,
                &row.model,
            );
            Some(Box::new(p))
        }
        "anthropic" => {
            let p = anthropic::AnthropicProvider::new_direct(&row.api_key, &row.model);
            Some(Box::new(p))
        }
        _ => None,
    }
}

/// 统一选择入口：优先从 DB 取已注册的 provider，DB 为空时回退到旧的
/// config.toml 逻辑（兼容升级用户）。
pub fn select_provider_unified(
    db: &Db,
    config: &LlmConfig,
    memex_dir: &Path,
) -> Option<Box<dyn LlmProvider>> {
    if let Ok(rows) = db.provider_list() {
        if !rows.is_empty() {
            return select_provider_from_db(db);
        }
    }
    select_provider(config, memex_dir)
}

/// 旧版选择逻辑（config.toml 路径），保留给无 DB 的场景（CLI、测试）。
pub fn select_provider(config: &LlmConfig, memex_dir: &Path) -> Option<Box<dyn LlmProvider>> {
    if config.ollama_enabled {
        let ollama = ollama::OllamaProvider::from_config(config);
        if ollama.is_available() {
            return Some(Box::new(ollama));
        }
    }

    if config.deepseek_enabled {
        if let Some(provider) = build_deepseek_provider(config, memex_dir) {
            if provider.is_available() {
                return Some(Box::new(provider));
            }
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

pub fn build_deepseek_provider(
    config: &LlmConfig,
    memex_dir: &Path,
) -> Option<openai_compat::OpenAiCompatProvider> {
    let creds = credentials::Credentials::load(memex_dir).ok()?;
    let key = creds.resolve_deepseek_key()?;
    let model = creds
        .resolve_deepseek_model()
        .unwrap_or_else(|| config.deepseek_model.clone());
    Some(openai_compat::OpenAiCompatProvider::new(
        "deepseek",
        "https://api.deepseek.com/v1",
        &key,
        &model,
    ))
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
            deepseek_enabled: false,
            deepseek_model: "deepseek-chat".into(),
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
            deepseek: None,
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
            deepseek: None,
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
