use std::time::Instant;

use memex_core::llm::build_provider_from_row;
use memex_core::llm::openai_compat::OpenAiCompatProvider;
use memex_core::llm::provider::LlmRequest;
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::db::providers::{LlmProviderRow, LlmProviderUpsert};
use serde::Serialize;

use super::error::{CmdError, CmdResult};

fn open_db() -> CmdResult<Db> {
    let db_path = memex_dir().join("memex.db");
    Ok(Db::open(&db_path)?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
    pub response_text: Option<String>,
}

/// Provider 测试用的最小请求。
///
/// `max_tokens` 设成 256 而不是 8 —— 8 token 对普通模型够，但对 DeepSeek-R1 /
/// V4 等 reasoning model **必失败**：它们把推理过程放在 `reasoning_content`,
/// 8 token 在思考阶段就耗尽，最终 `content` 是空字符串，触发 "empty content"
/// 误报。256 token 既能让常规模型快速回 "OK"，又给 reasoning model 留出
/// 完成简短推理 + 输出最终答案的余量。
fn micro_request() -> LlmRequest {
    LlmRequest {
        prompt: "Reply with exactly one word: OK".to_string(),
        system: None,
        max_tokens: 256,
        temperature: 0.0,
    }
}

#[tauri::command]
pub async fn llm_provider_list() -> CmdResult<Vec<LlmProviderRow>> {
    let db = open_db()?;
    Ok(db.provider_list()?)
}

#[tauri::command]
pub async fn llm_provider_upsert(provider: LlmProviderUpsert) -> CmdResult<LlmProviderRow> {
    let db = open_db()?;
    Ok(db.provider_upsert(provider)?)
}

#[tauri::command]
pub async fn llm_provider_delete(id: String) -> CmdResult<u64> {
    let db = open_db()?;
    Ok(db.provider_delete(&id)?)
}

#[tauri::command]
pub async fn llm_provider_test(id: String) -> CmdResult<ProviderTestResult> {
    let db = open_db()?;
    let row = db
        .provider_get(&id)?
        .ok_or_else(|| CmdError::NotFound(format!("provider {} not found", id)))?;

    let provider = build_provider_from_row(&row)
        .ok_or_else(|| CmdError::Validation(format!("unknown provider kind: {}", row.kind)))?;

    let start = Instant::now();

    if !provider.is_available() {
        let elapsed = start.elapsed().as_millis() as u64;
        if let Err(e) = db.provider_update_status(&id, "error", Some(elapsed as i64)) {
            tracing::warn!(provider_id = %id, error = %e, "failed to record provider error status");
        }
        return Ok(ProviderTestResult {
            ok: false,
            latency_ms: elapsed,
            error: Some("Provider not reachable or missing API key".into()),
            response_text: None,
        });
    }

    match provider.generate(&micro_request()) {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            if let Err(e) = db.provider_update_status(&id, "ok", Some(elapsed as i64)) {
                tracing::warn!(provider_id = %id, error = %e, "failed to record provider ok status");
            }
            Ok(ProviderTestResult {
                ok: true,
                latency_ms: elapsed,
                error: None,
                response_text: Some(resp.text),
            })
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            if let Err(update_err) = db.provider_update_status(&id, "error", Some(elapsed as i64)) {
                tracing::warn!(provider_id = %id, error = %update_err, "failed to record provider error status");
            }
            Ok(ProviderTestResult {
                ok: false,
                latency_ms: elapsed,
                error: Some(e.to_string()),
                response_text: None,
            })
        }
    }
}

#[tauri::command]
pub async fn llm_provider_test_draft(
    name: String,
    kind: String,
    base_url: String,
    model: String,
    api_key: String,
) -> CmdResult<ProviderTestResult> {
    let row = LlmProviderRow {
        id: String::new(),
        name,
        kind,
        base_url,
        model,
        api_key,
        enabled: true,
        is_default: false,
        status: "untested".into(),
        latency_ms: None,
        updated_at: String::new(),
    };

    let provider = build_provider_from_row(&row)
        .ok_or_else(|| CmdError::Validation(format!("unknown provider kind: {}", row.kind)))?;

    let start = Instant::now();

    if !provider.is_available() {
        let elapsed = start.elapsed().as_millis() as u64;
        return Ok(ProviderTestResult {
            ok: false,
            latency_ms: elapsed,
            error: Some("Provider not reachable or missing API key".into()),
            response_text: None,
        });
    }

    match provider.generate(&micro_request()) {
        Ok(resp) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(ProviderTestResult {
                ok: true,
                latency_ms: elapsed,
                error: None,
                response_text: Some(resp.text),
            })
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            Ok(ProviderTestResult {
                ok: false,
                latency_ms: elapsed,
                error: Some(e.to_string()),
                response_text: None,
            })
        }
    }
}

#[tauri::command]
pub async fn llm_list_models(
    kind: String,
    base_url: String,
    api_key: String,
) -> CmdResult<Vec<String>> {
    // ureq 是同步阻塞客户端；如果直接在 async fn 里调用会阻住整个 tokio runtime，
    // 让 IPC 排队、UI 看起来卡死（用户感觉「拉取」按钮无响应、超 10s 才弹错）。
    // 用 spawn_blocking 把 HTTP 调用挪到专门的阻塞线程池上。
    tokio::task::spawn_blocking(move || run_list_models(&kind, &base_url, &api_key))
        .await
        .map_err(|e| CmdError::Backend(format!("internal: blocking task join error: {e}")))?
}

fn run_list_models(kind: &str, base_url: &str, api_key: &str) -> CmdResult<Vec<String>> {
    match kind {
        "openai_compat" | "anthropic" => {
            let provider = OpenAiCompatProvider::new("_probe", base_url, api_key, "");
            Ok(provider.list_models()?)
        }
        "ollama" => {
            let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
            let mut resp = ureq::get(&url)
                .call()
                .map_err(|e| CmdError::Backend(format!("Cannot reach Ollama: {}", e)))?;
            let val: serde_json::Value = resp
                .body_mut()
                .read_json()
                .map_err(|e| CmdError::Backend(format!("Failed to parse Ollama tags: {}", e)))?;
            let models = val["models"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect();
            Ok(models)
        }
        _ => Err(CmdError::Validation(format!(
            "unsupported kind for model listing: {}",
            kind
        ))),
    }
}
