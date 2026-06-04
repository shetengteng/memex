//! Reflect Tauri commands — 暴露给 Dashboard 的 Reflect tab。
//!
//! 三个能力：
//! 1. `reflect_list` 列出已有 reflect 行
//! 2. `reflect_get(scope_key)` 取某条 reflect 的完整 markdown + 结构化字段
//! 3. `reflect_run(period)` 调 LLM 生成新 reflect（同 CLI `reflect run`）
//!
//! 直接调 memex-core，**不 spawn CLI** —— 因为 LLM 调用可能要 30s+，
//! spawn 进程会让 stderr 输出不可见且 cancellation 困难；同进程调用
//! 更适合长任务，且可以在 Tauri runtime 里走 spawn_blocking 不阻塞 UI。

use serde::Serialize;

use memex_core::config::MemexConfig;
use memex_core::llm::select_provider_unified;
use memex_core::memex_dir;
use memex_core::reflect::{run_reflect, today_utc, ReflectPeriod};
use memex_core::storage::db::Db;

#[derive(Debug, Serialize)]
pub struct ReflectEntry {
    pub scope_key: String,
    pub title: Option<String>,
    pub digest_count: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ReflectDetail {
    pub scope_key: String,
    pub title: Option<String>,
    /// 完整 markdown（同存到 aggregate_summaries.summary）
    pub markdown: String,
    pub patterns: Vec<String>,
    pub open_loops: Vec<String>,
    pub digest_count: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ReflectRunResult {
    pub scope_key: String,
    pub period_label: String,
    pub digest_count: usize,
    pub markdown: String,
    pub shipped: Vec<String>,
    pub patterns: Vec<String>,
    pub open_loops: Vec<String>,
}

#[tauri::command]
pub async fn reflect_list() -> Result<Vec<ReflectEntry>, String> {
    tokio::task::spawn_blocking(|| {
        let memex = memex_dir();
        let db = Db::open(&memex.join("memex.db")).map_err(|e| e.to_string())?;
        let rows = db
            .list_aggregate_summaries("reflect", 100)
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| ReflectEntry {
                scope_key: r.scope_key,
                title: r.title,
                digest_count: r.session_count,
                created_at: r.created_at,
            })
            .collect())
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}

#[tauri::command]
pub async fn reflect_get(scope_key: String) -> Result<Option<ReflectDetail>, String> {
    tokio::task::spawn_blocking(move || {
        let memex = memex_dir();
        let db = Db::open(&memex.join("memex.db")).map_err(|e| e.to_string())?;
        let row = db
            .get_aggregate_summary("reflect", &scope_key)
            .map_err(|e| e.to_string())?;
        Ok(row.map(|r| ReflectDetail {
            scope_key: r.scope_key,
            title: r.title,
            markdown: r.summary,
            patterns: r.topics,
            open_loops: r.decisions,
            digest_count: r.session_count,
            created_at: r.created_at,
        }))
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}

#[tauri::command]
pub async fn reflect_run(period: String) -> Result<ReflectRunResult, String> {
    tokio::task::spawn_blocking(move || {
        let parsed = ReflectPeriod::parse(&period).map_err(|e| e.to_string())?;

        let memex = memex_dir();
        let db = Db::open(&memex.join("memex.db")).map_err(|e| e.to_string())?;
        let config = MemexConfig::load(&memex).unwrap_or_default();
        let provider = select_provider_unified(&db, &config.llm, &memex)
            .ok_or_else(|| "没有可用的 LLM provider（先到 Settings → LLM Providers 注册）".to_string())?;

        let artifacts = run_reflect(
            &db,
            provider.as_ref(),
            parsed,
            today_utc(),
            Some(memex.as_path()),
        )
        .map_err(|e| e.to_string())?;

        Ok(ReflectRunResult {
            scope_key: artifacts.scope_key,
            period_label: artifacts.period_label,
            digest_count: artifacts.digest_count,
            markdown: artifacts.markdown,
            shipped: artifacts.output.shipped,
            patterns: artifacts.output.patterns,
            open_loops: artifacts.output.open_loops,
        })
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
}
