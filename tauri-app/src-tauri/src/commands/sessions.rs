use serde::Serialize;
use tauri::{AppHandle, Emitter};

use memex_core::config::MemexConfig;
use memex_core::memex_dir;
use memex_core::storage::db::{Db, SessionDetail, SessionRow};

#[tauri::command]
pub async fn list_recent(limit: Option<usize>, offset: Option<usize>) -> Result<Vec<SessionRow>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.list_sessions_paged(limit.unwrap_or(20), offset.unwrap_or(0))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session(session_id: String) -> Result<Option<SessionDetail>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(None);
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.get_session_detail(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn retry_summary(session_id: String) -> Result<bool, String> {
    let dir = memex_dir();
    let db_path = dir.join("memex.db");
    if !db_path.exists() {
        return Err("Database not found".into());
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let config = MemexConfig::load(&dir).map_err(|e| e.to_string())?;
    let provider = memex_core::llm::select_provider_unified(&db, &config.llm, &dir)
        .ok_or_else(|| "No LLM provider available. Enable Ollama or configure a custom LLM provider.".to_string())?;

    let _ = db.delete_summary(&session_id, "L2_session");
    let ok = memex_core::ingest::summarize_session_by_id(&db, provider.as_ref(), &session_id);
    Ok(ok)
}

#[derive(Debug, Clone, Serialize)]
pub struct SummaryProgress {
    pub current: usize,
    pub total: usize,
    pub session_id: String,
    pub success: bool,
    pub done: bool,
}

#[tauri::command]
pub async fn batch_summarize(app: AppHandle) -> Result<usize, String> {
    let dir = memex_dir();
    let db_path = dir.join("memex.db");
    if !db_path.exists() {
        return Err("Database not found".into());
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let config = MemexConfig::load(&dir).map_err(|e| e.to_string())?;
    let provider = memex_core::llm::select_provider_unified(&db, &config.llm, &dir)
        .ok_or_else(|| "No LLM provider available. Enable Ollama or configure a custom LLM provider.".to_string())?;

    // 用户主动点「批量摘要」按钮 → 把过期 / 缺失的 L2 都补上，
    // 不应用冷却（cool_down_secs=0），避免「明明 LLM 已经配好却没补摘要」的尴尬。
    let ids = db
        .sessions_needing_summary(100, 0)
        .map_err(|e| e.to_string())?;
    let total = ids.len();

    if total == 0 {
        return Ok(0);
    }

    std::thread::spawn(move || {
        for (i, sid) in ids.iter().enumerate() {
            let ok = memex_core::ingest::summarize_session_by_id(&db, provider.as_ref(), sid);
            let _ = app.emit("summary-progress", SummaryProgress {
                current: i + 1,
                total,
                session_id: sid.clone(),
                success: ok,
                done: i + 1 == total,
            });
        }
    });

    Ok(total)
}
