use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use memex_core::config::MemexConfig;
use memex_core::memex_dir;
use memex_core::storage::db::{Db, SessionDetail, SessionRow};

/// 当前批量摘要任务的中断标志位。`AtomicBool::store(true)` 后，正在运行的
/// `batch_summarize` 工作线程会在下一次循环检查时退出，并 emit `summary-progress`
/// 给前端（aborted=true）。`OnceLock` 让我们不需要 lazy_static / once_cell。
static ABORT_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn abort_flag() -> &'static Arc<AtomicBool> {
    ABORT_FLAG.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

#[tauri::command]
pub async fn list_recent(
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<SessionRow>, String> {
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
    let provider =
        memex_core::llm::select_provider_unified(&db, &config.llm, &dir).ok_or_else(|| {
            "No LLM provider available. Enable Ollama or configure a custom LLM provider."
                .to_string()
        })?;

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
    pub aborted: bool,
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
    let provider =
        memex_core::llm::select_provider_unified(&db, &config.llm, &dir).ok_or_else(|| {
            "No LLM provider available. Enable Ollama or configure a custom LLM provider."
                .to_string()
        })?;

    // 用户主动点「批量摘要」按钮 → 把过期 / 缺失的 L2 都补上，
    // 不应用冷却（cool_down_secs=0），避免「明明 LLM 已经配好却没补摘要」的尴尬。
    let ids = db
        .sessions_needing_summary(100, 0)
        .map_err(|e| e.to_string())?;
    let total = ids.len();

    if total == 0 {
        return Ok(0);
    }

    // 启动新任务前清理上一轮可能未复位的 abort 标志
    let flag = abort_flag().clone();
    flag.store(false, Ordering::SeqCst);

    let interval_ms = config.llm.summarize_interval_ms;

    std::thread::spawn(move || {
        for (i, sid) in ids.iter().enumerate() {
            // 在调用 LLM 前先检查 abort —— 这样按钮按下后最多再等当前一条
            // 摘要跑完（无法打断已经发出去的 HTTP 请求），但绝不会再发起新的
            if flag.load(Ordering::SeqCst) {
                let _ = app.emit(
                    "summary-progress",
                    SummaryProgress {
                        current: i,
                        total,
                        session_id: sid.clone(),
                        success: false,
                        done: true,
                        aborted: true,
                    },
                );
                break;
            }

            let ok = memex_core::ingest::summarize_session_by_id(&db, provider.as_ref(), sid);
            let is_last = i + 1 == total;
            let _ = app.emit(
                "summary-progress",
                SummaryProgress {
                    current: i + 1,
                    total,
                    session_id: sid.clone(),
                    success: ok,
                    done: is_last,
                    aborted: false,
                },
            );

            // throttle：除最后一条外，每条摘要之间 sleep 配置好的间隔
            // 让 Ollama / Apple Silicon 有时间散热、UI 也能腾出渲染时间
            if !is_last && interval_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(interval_ms));
            }
        }
        flag.store(false, Ordering::SeqCst);
    });

    Ok(total)
}

/// 用户主动中断当前批量摘要任务。
/// 工作线程会在下一次循环开始时检测到该标志并退出。
#[tauri::command]
pub async fn abort_summarize() -> Result<bool, String> {
    let flag = abort_flag();
    let was_running = !flag.load(Ordering::SeqCst);
    flag.store(true, Ordering::SeqCst);
    Ok(was_running)
}
