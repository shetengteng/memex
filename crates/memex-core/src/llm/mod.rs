pub mod anthropic;
pub mod ollama;
pub mod openai_compat;
pub mod provider;
pub mod reflect;
pub mod summarize;
pub mod threads;

use std::path::Path;

use crate::config::LlmConfig;
use crate::storage::db::{Db, LlmProviderRow};
use provider::LlmProvider;

/// 从 DB 中已注册的 provider 列表构建 LLM client。
/// is_default 排在最前面，然后按 name 排序。
/// 逐个尝试，第一个 `is_available()` 的胜出。
pub fn select_provider_from_db(db: &Db) -> Option<Box<dyn LlmProvider>> {
    let rows = db.provider_list().ok()?;
    for row in rows.iter().filter(|r| r.enabled) {
        if let Some(p) = build_provider_from_row(row)
            && p.is_available()
        {
            return Some(p);
        }
    }
    None
}

/// 从一行 DB 记录构建具体的 LlmProvider 实例。
/// 支持的 kind：`ollama` / `openai_compat` / `anthropic`。
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

/// 统一选择入口：优先从 DB 中**已启用**的 provider 选一个可用的；
/// 若没有任何 enabled 的 DB provider（DB 为空、或所有 row 都 disabled），
/// 再回退到老的 `LlmConfig.ollama_*` 快捷入口。
///
/// 注意：此处用 "是否存在 enabled row" 而非 "DB 是否非空" 来判断。
/// 否则一旦用户在 DB 里关掉所有云端 provider（例如 DeepSeek 关掉但保留配置），
/// Ollama 老开关 (`config.llm.ollama_enabled`) 就会被静默屏蔽，
/// dashboard 会误报「LLM 摘要 未配置」。
pub fn select_provider_unified(
    db: &Db,
    config: &LlmConfig,
    memex_dir: &Path,
) -> Option<Box<dyn LlmProvider>> {
    if let Ok(rows) = db.provider_list()
        && rows.iter().any(|r| r.enabled)
    {
        return select_provider_from_db(db);
    }
    select_provider(config, memex_dir)
}

/// 仅依据 `LlmConfig` 中的 Ollama 老快捷入口选择 provider。
/// DeepSeek / Anthropic / 其他云端 provider 一律走 DB providers，
/// 不再从 config.toml 或 credentials.toml 中读取。
pub fn select_provider(config: &LlmConfig, _memex_dir: &Path) -> Option<Box<dyn LlmProvider>> {
    if config.ollama_enabled {
        let ollama = ollama::OllamaProvider::from_config(config);
        if ollama.is_available() {
            return Some(Box::new(ollama));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::providers::LlmProviderUpsert;
    use tempfile::TempDir;

    fn disabled_config() -> LlmConfig {
        LlmConfig {
            ollama_enabled: false,
            ollama_url: "http://127.0.0.1:0".into(),
            ollama_model: "test".into(),
            summary_cooldown_secs: 600,
            summarize_interval_ms: 0,
        }
    }

    #[test]
    fn returns_none_when_all_paths_disabled() {
        let tmp = TempDir::new().unwrap();
        let provider = select_provider(&disabled_config(), tmp.path());
        assert!(provider.is_none());
    }

    #[test]
    fn returns_none_when_ollama_unreachable() {
        let tmp = TempDir::new().unwrap();
        let cfg = LlmConfig {
            ollama_enabled: true,
            ollama_url: "http://127.0.0.1:0".into(),
            ollama_model: "test".into(),
            summary_cooldown_secs: 600,
            summarize_interval_ms: 0,
        };
        let provider = select_provider(&cfg, tmp.path());
        assert!(
            provider.is_none(),
            "ollama_url 指向不可达端口时，不应该返回 provider"
        );
    }

    /// 回归：用户开了 Ollama 老开关，但 DB 里有一条 disabled 的云端 provider。
    /// 旧实现在 DB 非空时直接走 DB 分支并 return None，会让 Ollama 老开关被静默屏蔽，
    /// dashboard 误报「LLM 摘要 未配置」。
    ///
    /// 验证策略：构造一个本地 HTTP server 假装是 Ollama 的 `/api/tags`，
    /// 返回带 model 的列表。这样 ollama provider 的 `is_available()` 会返回 true，
    /// 我们就能断言 `select_provider_unified` 在「DB 全 disabled」时确实走到了 ollama 分支。
    #[test]
    fn unified_falls_back_to_ollama_when_all_db_providers_disabled() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ollama mock");
        let port = listener.local_addr().unwrap().port();

        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let body = r#"{"models":[{"name":"test:latest"}]}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });

        let tmp = TempDir::new().unwrap();
        let db = Db::open(&tmp.path().join("t.db")).unwrap();
        db.provider_upsert(LlmProviderUpsert {
            id: "ds".into(),
            name: "DeepSeek".into(),
            kind: "openai_compat".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            model: "deepseek-chat".into(),
            api_key: "sk-test".into(),
            enabled: false,
            is_default: false,
        })
        .unwrap();

        let cfg = LlmConfig {
            ollama_enabled: true,
            ollama_url: format!("http://127.0.0.1:{}", port),
            ollama_model: "test".into(),
            summary_cooldown_secs: 600,
            summarize_interval_ms: 0,
        };

        let provider = select_provider_unified(&db, &cfg, tmp.path());
        assert!(
            provider.is_some(),
            "DB 只有 disabled 的 provider + ollama_enabled=true 时，应该回退到 Ollama"
        );
        assert_eq!(provider.unwrap().name(), "ollama");
    }
}
