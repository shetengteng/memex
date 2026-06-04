use memex_core::config::ensure_memex_dir;
use memex_core::ingest::run_ingest;
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct IngestResult {
    pub messages_ingested: u64,
    pub chunks_created: u64,
}

/// 触发一次扫描 + ingest。`adapter` 不传则扫描所有 enabled adapter；
/// 传具体 key（claude_code / cursor / codex / opencode / aider / continue_dev / cline）
/// 则只扫这一个。前端把单个 adapter 行的扫描按钮挂到这里。
///
/// 注意：`continue_dev` 是前端 / config / toggle_adapter 历史沿用的命名，
/// 但 collector 自己的 name() 是 `"continue"`（见 `collector/continue_dev.rs`）。
/// 这里做一次映射，避免把映射逻辑下沉到 ingest 层。
#[tauri::command]
pub async fn trigger_ingest(adapter: Option<String>) -> Result<IngestResult, String> {
    let mapped = adapter.map(|a| if a == "continue_dev" { "continue".to_string() } else { a });
    tokio::task::spawn_blocking(move || {
        let memex = memex_dir();
        ensure_memex_dir(&memex).map_err(|e| e.to_string())?;

        let db_path = memex.join("memex.db");
        let db = Db::open(&db_path).map_err(|e| e.to_string())?;

        let result = run_ingest(&db, &memex, mapped.as_deref()).map_err(|e| e.to_string())?;
        Ok(IngestResult {
            messages_ingested: result.messages_ingested,
            chunks_created: result.chunks_created,
        })
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}
