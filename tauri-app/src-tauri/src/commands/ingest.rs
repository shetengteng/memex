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

/// 触发一次全量扫描 + ingest。会扫描所有已启用 adapter 的会话目录
/// （Claude Code / Cursor / Codex / OpenCode 等），把新消息写入 `memex.db`。
/// 由于 ingest 是同步、可能耗时较长（首次安装时可达数十秒甚至更久），
/// 这里放到 spawn_blocking 里跑，避免阻塞 Tauri 的 async runtime。
#[tauri::command]
pub async fn trigger_ingest() -> Result<IngestResult, String> {
    tokio::task::spawn_blocking(|| {
        let memex = memex_dir();
        ensure_memex_dir(&memex).map_err(|e| e.to_string())?;

        let db_path = memex.join("memex.db");
        let db = Db::open(&db_path).map_err(|e| e.to_string())?;

        let result = run_ingest(&db, &memex, None).map_err(|e| e.to_string())?;
        Ok(IngestResult {
            messages_ingested: result.messages_ingested,
            chunks_created: result.chunks_created,
        })
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}
