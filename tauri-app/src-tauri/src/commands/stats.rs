use memex_core::config::MemexConfig;
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use memex_core::storage::queries::{ProjectSummary, StatsBreakdown, TimelineEntry};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Stats {
    pub sessions: u64,
    pub messages: u64,
    pub chunks: u64,
    pub db_exists: bool,
    pub summaries: u64,
    /// 实际可以生成 L2 摘要的会话数（即 messages >= 2 的会话）。
    /// UI 用它当摘要进度条的分母，否则永远卡在 < 100%。
    pub sessions_eligible_for_summary: u64,
    pub chunks_summarized: u64,
    pub llm_provider: Option<String>,
}

#[tauri::command]
pub async fn get_stats() -> Result<Stats, String> {
    let dir = memex_dir();
    let db_path = dir.join("memex.db");
    if !db_path.exists() {
        return Ok(Stats {
            sessions: 0,
            messages: 0,
            chunks: 0,
            db_exists: false,
            summaries: 0,
            sessions_eligible_for_summary: 0,
            chunks_summarized: 0,
            llm_provider: None,
        });
    }

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    let config = MemexConfig::load(&dir).unwrap_or_default();
    // 用 *_unified 版本，让 DB 中已注册的自定义 provider（DeepSeek/OpenAI/Anthropic 等）
    // 也能在 dashboard 上显示「已启用」，而不仅看 LlmConfig 的 ollama 老开关。
    let provider_name = memex_core::llm::select_provider_unified(&db, &config.llm, &dir)
        .map(|p| p.name().to_string());

    Ok(Stats {
        sessions: db.session_count().unwrap_or(0),
        messages: db.message_count().unwrap_or(0),
        chunks: db.chunk_count().unwrap_or(0),
        db_exists: true,
        summaries: db.summary_count().unwrap_or(0),
        sessions_eligible_for_summary: db.sessions_eligible_for_summary_count().unwrap_or(0),
        chunks_summarized: db.chunks_with_summary_count().unwrap_or(0),
        llm_provider: provider_name,
    })
}

#[tauri::command]
pub async fn get_breakdown() -> Result<StatsBreakdown, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(StatsBreakdown {
            by_adapter: Default::default(),
            by_project: Default::default(),
            recent_7d_sessions: 0,
            recent_7d_messages: 0,
            recent_30d_sessions: 0,
            recent_30d_messages: 0,
        });
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.stats_breakdown().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_timeline(days: Option<u32>) -> Result<Vec<TimelineEntry>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.timeline(days.unwrap_or(30)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_projects() -> Result<Vec<ProjectSummary>, String> {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return Ok(vec![]);
    }
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
    db.list_project_summaries().map_err(|e| e.to_string())
}
