use std::path::PathBuf;

use memex_core::config::MemexConfig;
use memex_core::ingest::{
    regenerate_daily_report, regenerate_monthly_report, regenerate_report_by_key,
    regenerate_weekly_report,
};
use memex_core::llm::select_provider_unified;
use memex_core::memex_dir;
use memex_core::storage::db::{AggregateSummaryRow, Db};

use super::error::{CmdError, CmdResult};

#[tauri::command]
pub async fn list_reports(
    scope: String,
    limit: Option<u32>,
) -> CmdResult<Vec<AggregateSummaryRow>> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let db = Db::open(&db_path)?;
    let rows = db.list_aggregate_summaries(&scope, limit.unwrap_or(60))?;
    Ok(rows)
}

/// scope: "daily" | "weekly" | "monthly" — 生成最新的
/// scope_key: 可选，如 "daily:2026-06-04" / "monthly:2026-06" — 重新生成指定的
#[tauri::command]
pub async fn regenerate_report(
    scope: String,
    scope_key: Option<String>,
) -> CmdResult<Option<AggregateSummaryRow>> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    if !db_path.exists() {
        return Err(CmdError::NotFound(
            "memex.db 不存在，请先运行 memex ingest".into(),
        ));
    }
    let db = Db::open(&db_path)?;
    let cfg = MemexConfig::load(&memex)?;
    let provider = select_provider_unified(&db, &cfg.llm, &memex).ok_or_else(|| {
        CmdError::Backend("未配置 LLM 提供方，请在设置中启用 Ollama 或自定义 LLM 提供商".into())
    })?;

    if let Some(key) = scope_key {
        return Ok(regenerate_report_by_key(&db, provider.as_ref(), &key)?);
    }

    let row = match scope.as_str() {
        "daily" => regenerate_daily_report(&db, provider.as_ref())?,
        "weekly" => regenerate_weekly_report(&db, provider.as_ref())?,
        "monthly" => regenerate_monthly_report(&db, provider.as_ref())?,
        other => {
            return Err(CmdError::Validation(format!("不支持的报告类型：{}", other)));
        }
    };
    Ok(row)
}

/// 把任意文本（通常是 markdown）原样写入用户在 save dialog 里选的本地路径。
/// 仅 UI 触发的导出走这里——前端已经在内存里把报告渲染好了，后端只负责落盘。
#[tauri::command]
pub async fn export_text_file(target_path: String, content: String) -> CmdResult<u64> {
    let path = PathBuf::from(&target_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CmdError::Backend(format!("创建目录 {} 失败: {}", parent.display(), e))
            })?;
        }
    }
    std::fs::write(&path, &content).map_err(|e| {
        CmdError::Backend(format!("写入 {} 失败: {}", path.display(), e))
    })?;
    Ok(content.len() as u64)
}
