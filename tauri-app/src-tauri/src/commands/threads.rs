//! Tauri commands: L5「主题线索」相关 IPC。
//!
//! 五个命令：
//! - `list_threads(limit, offset)`：拉线索列表（分页），按 updated_at DESC。
//! - `get_thread_detail(thread_id)`：拉单条线索详情 + 命中的 session 列表。
//! - `regenerate_threads()`：手动触发 LLM 全量聚类，写入/更新 threads 表。
//! - `delete_thread(thread_id)`：物理删除一条线索（及其 thread_sessions 关联）。
//! - `search_thread_by_query(query)`：按关键词让 LLM 在所有 L2 摘要里挑出
//!   相关 session，作为新线索落库；返回新线索的 thread_id。
//!
//! 设计：与 reports.rs 一样在每个 command 内部 `Db::open` —— 单文件 SQLite，
//! 全程进程内复用 WAL，开销可忽略。

use memex_core::config::MemexConfig;
use memex_core::ingest::{
    regenerate_threads as core_regenerate_threads,
    search_thread_by_query as core_search_thread_by_query,
};
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

/// 物理删除一条线索（及其 thread_sessions 关联，靠 FK CASCADE）。
/// 用户在"测试版本"阶段可能反复迭代，需要快速干掉脏数据。
#[tauri::command]
pub async fn delete_thread(thread_id: i64) -> Result<(), String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Err("memex.db 不存在".to_string());
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.delete_thread(thread_id).map_err(|e| e.to_string())?;
    Ok(())
}

/// 按关键词在所有 L2 摘要里 LLM 检索相关 session，作为新线索落库。
/// 与 regenerate_threads 的区别：那个是全量自动聚类，这个是"按主题词命题搜索"。
#[tauri::command]
pub async fn search_thread_by_query(query: String) -> Result<i64, String> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    if !db_path.exists() {
        return Err("memex.db 不存在，请先运行 memex ingest".to_string());
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let cfg = MemexConfig::load(&memex).map_err(|e| e.to_string())?;
    let provider = select_provider_unified(&db, &cfg.llm, &memex)
        .ok_or_else(|| "未配置 LLM 提供方，请在设置中启用 Ollama 或自定义 LLM 提供商".to_string())?;
    let q = query.trim();
    if q.is_empty() {
        return Err("查询关键词不能为空".to_string());
    }
    core_search_thread_by_query(&db, provider.as_ref(), q).map_err(|e| e.to_string())
}
