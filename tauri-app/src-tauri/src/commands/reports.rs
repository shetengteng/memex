use memex_core::config::MemexConfig;
use memex_core::ingest::{regenerate_daily_report, regenerate_weekly_report, regenerate_report_by_key};
use memex_core::llm::select_provider_unified;
use memex_core::memex_dir;
use memex_core::storage::db::{AggregateSummaryRow, Db};

#[tauri::command]
pub async fn list_reports(scope: String, limit: Option<u32>) -> Result<Vec<AggregateSummaryRow>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.list_aggregate_summaries(&scope, limit.unwrap_or(60))
        .map_err(|e| e.to_string())
}

/// scope: "daily" | "weekly" — 生成最新的
/// scope_key: 可选，如 "daily:2026-06-04" — 重新生成指定日期的
#[tauri::command]
pub async fn regenerate_report(scope: String, scope_key: Option<String>) -> Result<Option<AggregateSummaryRow>, String> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    if !db_path.exists() {
        return Err("memex.db 不存在，请先运行 memex ingest".to_string());
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let cfg = MemexConfig::load(&memex).map_err(|e| e.to_string())?;
    let provider = select_provider_unified(&db, &cfg.llm, &memex)
        .ok_or_else(|| "未配置 LLM 提供方，请在设置中启用 Ollama 或自定义 LLM 提供商".to_string())?;

    if let Some(key) = scope_key {
        return regenerate_report_by_key(&db, provider.as_ref(), &key)
            .map_err(|e| e.to_string());
    }

    match scope.as_str() {
        "daily" => regenerate_daily_report(&db, provider.as_ref()).map_err(|e| e.to_string()),
        "weekly" => regenerate_weekly_report(&db, provider.as_ref()).map_err(|e| e.to_string()),
        other => Err(format!("不支持的报告类型：{}", other)),
    }
}
