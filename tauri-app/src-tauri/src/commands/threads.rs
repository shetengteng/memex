//! Tauri commands: L5「主题线索」相关 IPC。
//!
//! 三个命令：
//! - `list_threads(limit, offset)`：拉线索列表（分页），按 updated_at DESC。
//! - `get_thread_detail(thread_id)`：拉单条线索详情 + 命中的 session 列表。
//! - `regenerate_threads()`：手动触发 LLM 聚类，写入/更新 threads 表。
//!
//! 设计：与 reports.rs 一样在每个 command 内部 `Db::open` —— 单文件 SQLite，
//! 全程进程内复用 WAL，开销可忽略。

use memex_core::config::MemexConfig;
use memex_core::ingest::regenerate_threads as core_regenerate_threads;
use memex_core::llm::select_provider_unified;
use memex_core::memex_dir;
use memex_core::storage::db::{Db, ThreadDetail, ThreadRow};

#[tauri::command]
pub async fn list_threads(
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ThreadRow>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.list_threads_paged(
        limit.unwrap_or(100) as usize,
        offset.unwrap_or(0) as usize,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_thread_detail(thread_id: i64) -> Result<Option<ThreadDetail>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.get_thread_detail(thread_id).map_err(|e| e.to_string())
}

/// 手动触发 L5 聚类。返回新建/更新的 thread 数量。
/// 阻塞调用（聚类是单次 LLM 调用），前端按 isPending 显示 spinner 即可。
#[tauri::command]
pub async fn regenerate_threads() -> Result<usize, String> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    if !db_path.exists() {
        return Err("memex.db 不存在，请先运行 memex ingest".to_string());
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let cfg = MemexConfig::load(&memex).map_err(|e| e.to_string())?;
    let provider = select_provider_unified(&db, &cfg.llm, &memex)
        .ok_or_else(|| "未配置 LLM 提供方，请在设置中启用 Ollama 或自定义 LLM 提供商".to_string())?;
    core_regenerate_threads(&db, provider.as_ref()).map_err(|e| e.to_string())
}
