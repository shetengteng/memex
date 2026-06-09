use std::time::Instant;

use memex_core::llm::build_provider_from_row;
use memex_core::llm::openai_compat::OpenAiCompatProvider;
use memex_core::llm::provider::LlmRequest;
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::db::providers::{LlmProviderRow, LlmProviderUpsert};
use serde::Serialize;

fn open_db() -> Result<Db, String> {
    let db_path = memex_dir().join("memex.db");
    Db::open(&db_path).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
    pub response_text: Option<String>,
}

fn micro_request() -> LlmRequest {
    LlmRequest {
        prompt: "Reply with exactly one word: OK".to_string(),
        system: None,
        max_tokens: 8,
        temperature: 0.0,
    }
}

#[tauri::command]
pub async fn llm_provider_list() -> Result<Vec<LlmProviderRow>, String> {
    let db = open_db()?;
    db.provider_list().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_provider_upsert(provider: LlmProviderUpsert) -> Result<LlmProviderRow, String> {
    let db = open_db()?;
    db.provider_upsert(provider).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_provider_delete(id: String) -> Result<u64, String> {
    let db = open_db()?;
    db.provider_delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn llm_provider_test(id: String) -> Result<ProviderTestResult, String> {
    let db = open_db()?;
    let row = db
        .provider_get(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("provider {} not found", id))?;

    let provider = build_provider_from_row(&row)
        .ok_or_else(|| format!("unknown provider kind: {}", row.kind))?;

    let start = Instant::now();

    if !provider.is_available() {
        let elapsed = start.elapsed().as_millis() as u64;
        let _ = db.provider_update_status(&id, "error", Some(elapsed as i64));
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
            let _ = db.provider_update_status(&id, "ok", Some(elapsed as i64));
            Ok(ProviderTestResult {
                ok: true,
                latency_ms: elapsed,
                error: None,
                response_text: Some(resp.text),
            })
        }
        Err(e) => {
            let elapsed = start.elapsed().as_millis() as u64;
            let _ = db.provider_update_status(&id, "error", Some(elapsed as i64));
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
) -> Result<ProviderTestResult, String> {
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
        .ok_or_else(|| format!("unknown provider kind: {}", row.kind))?;

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
) -> Result<Vec<String>, String> {
    // ureq 是同步阻塞客户端；如果直接在 async fn 里调用会阻住整个 tokio runtime，
    // 让 IPC 排队、UI 看起来卡死（用户感觉「拉取」按钮无响应、超 10s 才弹错）。
    // 用 spawn_blocking 把 HTTP 调用挪到专门的阻塞线程池上。
    tokio::task::spawn_blocking(move || run_list_models(&kind, &base_url, &api_key))
        .await
        .map_err(|e| format!("internal: blocking task join error: {e}"))?
}

fn run_list_models(kind: &str, base_url: &str, api_key: &str) -> Result<Vec<String>, String> {
    match kind {
        "openai_compat" | "anthropic" => {
            let provider = OpenAiCompatProvider::new("_probe", base_url, api_key, "");
            provider.list_models().map_err(|e| e.to_string())
        }
        "ollama" => {
            let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
            let mut resp = ureq::get(&url)
                .call()
                .map_err(|e| format!("Cannot reach Ollama: {}", e))?;
            let val: serde_json::Value = resp
                .body_mut()
                .read_json()
                .map_err(|e| format!("Failed to parse Ollama tags: {}", e))?;
            let models = val["models"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|m| m["name"].as_str().map(String::from))
                .collect();
            Ok(models)
        }
        _ => Err(format!("unsupported kind for model listing: {}", kind)),
    }
}
