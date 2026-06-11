use std::sync::Arc;
use std::time::Instant;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use memex_core::context::{ContextOptions, build_context, search_by_project};
use memex_core::ingest;
use memex_core::retriever::{Retriever, SearchFilter};
use memex_core::storage::db::Db;
use memex_core::storage::metrics::MetricEntry;

pub type AppState = Arc<Db>;

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_ms": 0,
    }))
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub adapter: Option<String>,
    pub project: Option<String>,
    pub chunk_type: Option<String>,
    pub after: Option<String>,
    pub before: Option<String>,
}

fn default_limit() -> usize {
    10
}

pub async fn search(
    State(db): State<AppState>,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let filter = SearchFilter {
        adapter: params.adapter,
        project: params.project,
        session_id: None,
        chunk_type: params.chunk_type,
        after: params.after,
        before: params.before,
    };
    let started = Instant::now();
    let retriever = Retriever::new(&db);
    match retriever.search_filtered(&params.q, params.limit, &filter) {
        Ok(results) => {
            let latency_ms = started.elapsed().as_millis() as u64;
            let _ = db.write_access_log(&params.q, results.len(), latency_ms);
            let _ = db.record_search_latency(latency_ms);
            Json(serde_json::json!({ "results": results })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

pub async fn list_sessions(
    State(db): State<AppState>,
    Query(params): Query<ListSessionsParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(20);
    match db.list_sessions(limit) {
        Ok(sessions) => Json(serde_json::json!({ "sessions": sessions })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct ListSessionsParams {
    pub limit: Option<usize>,
}

pub async fn get_session(State(db): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match db.get_session_detail(&id) {
        Ok(Some(detail)) => Json(serde_json::json!(detail)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "session not found"
            })),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

/// 给前端 / CLI 的统一统计端点。
///
/// 4 个 totals 是 db 全表计数；`today` 和 `last_7_days` 是 metrics 表里按日
/// 累积的运行时计数（search_count / mcp_calls / ingest_count 等）。CLI 的
/// `memex stats` 命令、Tauri menubar 都消费同一份 JSON，避免出现"GUI 看到
/// 一份数字、CLI 看到另一份"的语义漂移。
#[derive(Serialize)]
struct StatsResponse {
    sessions: u64,
    messages: u64,
    chunks: u64,
    sources: u64,
    /// metric_name → value，仅当日的累积值。空对象表示今天还没有任何活动。
    today: serde_json::Map<String, serde_json::Value>,
    /// 过去 N 天（含今天）所有 metric 的求和。窗口大小由 `STATS_RANGE_DAYS` 控制。
    last_7_days: serde_json::Map<String, serde_json::Value>,
}

/// 与 CLI `commands::stats::ACTIVITY_WINDOW_DAYS` 保持一致，避免两端窗口大小漂移。
const STATS_RANGE_DAYS: u32 = 7;

pub async fn stats(State(db): State<AppState>) -> impl IntoResponse {
    let resp = StatsResponse {
        sessions: db.session_count().unwrap_or(0),
        messages: db.message_count().unwrap_or(0),
        chunks: db.chunk_count().unwrap_or(0),
        sources: db.source_count().unwrap_or(0),
        today: metric_map(&db.get_today_metrics().unwrap_or_default()),
        last_7_days: aggregate_range(&db, STATS_RANGE_DAYS),
    };
    Json(resp)
}

fn metric_map(entries: &[MetricEntry]) -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    for e in entries {
        map.insert(e.name.clone(), serde_json::Value::from(e.value));
    }
    map
}

/// 把 `get_metrics_range` 拉回的 daily buckets 合并成 metric_name → 总和。
fn aggregate_range(db: &Db, days: u32) -> serde_json::Map<String, serde_json::Value> {
    let mut totals: std::collections::BTreeMap<String, i64> = Default::default();
    let Ok(daily) = db.get_metrics_range(days) else {
        return serde_json::Map::new();
    };
    for day in daily {
        for entry in day.entries {
            *totals.entry(entry.name).or_insert(0) += entry.value;
        }
    }
    let mut map = serde_json::Map::new();
    for (name, value) in totals {
        map.insert(name, serde_json::Value::from(value));
    }
    map
}

#[derive(Deserialize)]
pub struct ConfigBody {
    pub key: String,
    pub value: Option<String>,
}

pub async fn get_config(
    State(db): State<AppState>,
    Query(params): Query<ConfigKeyParam>,
) -> impl IntoResponse {
    match db.kv_get(&params.key) {
        Ok(Some(v)) => Json(serde_json::json!({ "key": params.key, "value": v })).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "key not found"
            })),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct ConfigKeyParam {
    pub key: String,
}

pub async fn set_config(
    State(db): State<AppState>,
    Json(body): Json<ConfigBody>,
) -> impl IntoResponse {
    let value = body.value.unwrap_or_default();
    match db.kv_set(&body.key, &value) {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

fn err_body(e: &anyhow::Error) -> serde_json::Value {
    serde_json::json!({ "error": e.to_string() })
}

#[derive(Deserialize)]
pub struct TimelineParams {
    #[serde(default = "default_timeline_days")]
    pub days: u32,
}

fn default_timeline_days() -> u32 {
    30
}

pub async fn timeline(
    State(db): State<AppState>,
    Query(params): Query<TimelineParams>,
) -> impl IntoResponse {
    match db.timeline(params.days) {
        Ok(entries) => {
            let mut grouped: std::collections::BTreeMap<String, serde_json::Value> =
                std::collections::BTreeMap::new();
            for e in &entries {
                let day = grouped.entry(e.date.clone()).or_insert_with(|| {
                    serde_json::json!({
                        "date": e.date,
                        "sessions": 0i64,
                        "messages": 0i64,
                        "by_adapter": {}
                    })
                });
                if let Some(obj) = day.as_object_mut() {
                    *obj.entry("sessions").or_insert(serde_json::json!(0)) =
                        serde_json::json!(obj["sessions"].as_i64().unwrap_or(0) + e.sessions);
                    *obj.entry("messages").or_insert(serde_json::json!(0)) =
                        serde_json::json!(obj["messages"].as_i64().unwrap_or(0) + e.messages);
                    let adapters = obj
                        .entry("by_adapter")
                        .or_insert_with(|| serde_json::json!({}));
                    if let Some(a) = adapters.as_object_mut() {
                        a.insert(e.adapter.clone(), serde_json::json!(e.sessions));
                    }
                }
            }
            let timeline: Vec<serde_json::Value> = grouped.into_values().rev().collect();
            Json(serde_json::json!({ "timeline": timeline })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

pub async fn stats_breakdown(State(db): State<AppState>) -> impl IntoResponse {
    match db.stats_breakdown() {
        Ok(breakdown) => Json(serde_json::json!(breakdown)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

pub async fn summary_stats(State(db): State<AppState>) -> impl IntoResponse {
    let session_summaries = db.summary_count().unwrap_or(0);
    let chunk_summaries = db.chunks_with_summary_count().unwrap_or(0);
    let sessions_total = db.session_count().unwrap_or(0);
    let sessions_eligible_for_summary = db.sessions_eligible_for_summary_count().unwrap_or(0);
    let chunks_total = db.chunk_count().unwrap_or(0);

    let memex_dir = memex_core::memex_dir();
    let config = memex_core::config::MemexConfig::load(&memex_dir).unwrap_or_default();
    // 用 *_unified 版本以便把 DB 中的自定义 provider（OpenAI/Anthropic/DeepSeek 等）
    // 也算作「LLM 启用」，否则仅看 Ollama 的老配置会让有 DB provider 的用户
    // 在 menubar 上看到「LLM 摘要 未配置」。
    let provider = memex_core::llm::select_provider_unified(&db, &config.llm, &memex_dir)
        .map(|p| p.name().to_string());

    Json(serde_json::json!({
        "session_summaries": session_summaries,
        "chunk_summaries": chunk_summaries,
        "sessions_total": sessions_total,
        "sessions_eligible_for_summary": sessions_eligible_for_summary,
        "chunks_total": chunks_total,
        "llm_provider": provider,
        "ollama_enabled": config.llm.ollama_enabled,
        "ollama_model": config.llm.ollama_model,
    }))
}

pub async fn get_session_summary(
    State(db): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db.get_summary(&id, "L2_session") {
        Ok(Some(summary)) => Json(serde_json::json!(summary)).into_response(),
        Ok(None) => Json(serde_json::json!(null)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct ContextParams {
    /// 调用方 cwd。可选：当 caller 没法提供 cwd（例如 IDE 通过 MCP 触发）时
    /// daemon 会回退到 `std::env::current_dir`，跟 CLI `memex context` 行为一致。
    pub cwd: Option<String>,
    /// 显式项目路径，优先级高于 `cwd`。
    pub project: Option<String>,
    /// 最近会话数；缺省 3。
    pub top: Option<usize>,
    /// 是否脱敏；缺省 true（MCP 路径默认脱敏，跟 daemon 内部 hook 路径一致）。
    pub redact: Option<bool>,
}

/// GET /context —— 返回项目工作记忆的 TARS-style markdown。
///
/// 该端点被 memex-mcp 的 `get_project_context` 工具调用。CLI 的
/// `memex context` 不走这条路径（hook 高频路径，daemon 关机时仍需可用）。
pub async fn context(
    State(db): State<AppState>,
    Query(params): Query<ContextParams>,
) -> impl IntoResponse {
    let top = params.top.unwrap_or(3);
    let redact = params.redact.unwrap_or(true);

    let project_path = if let Some(p) = params.project {
        p
    } else {
        let cwd = match params.cwd {
            Some(s) => std::path::PathBuf::from(s),
            None => match std::env::current_dir() {
                Ok(c) => c,
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, Json(err_msg(&e.to_string())))
                        .into_response();
                }
            },
        };
        match search_by_project(&db, &cwd) {
            Ok(Some(m)) => m.project_path,
            Ok(None) => {
                let banner = format!(
                    "**Memex 工作记忆**\n\n当前目录 {} 暂无关联项目会话；新的 AI 会话会被自动采集。",
                    cwd.display()
                );
                return Json(serde_json::json!({
                    "project_path": serde_json::Value::Null,
                    "markdown": banner,
                }))
                .into_response();
            }
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
        }
    };

    let opts = ContextOptions { top_n: top, redact };
    match build_context(&db, &project_path, &opts) {
        Ok(md) => Json(serde_json::json!({
            "project_path": project_path,
            "markdown": md,
        }))
        .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

#[derive(Deserialize)]
pub struct SessionsRangeParams {
    pub after: String,
    pub before: String,
    pub limit: Option<usize>,
    pub project: Option<String>,
}

/// GET /sessions/range —— 给 MCP `list_sessions_by_range` 工具用。
///
/// 跟 CLI 的"by date range"查询同源，但 daemon 这里负责一次性把每条 session
/// 关联的 L2 summary 内嵌进去，免得 mcp 端再多打 N 次 HTTP。
pub async fn sessions_range(
    State(db): State<AppState>,
    Query(params): Query<SessionsRangeParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    let after = normalize_date_bound(&params.after, true);
    let before = normalize_date_bound(&params.before, false);

    let sessions = match db.list_sessions_in_range(&after, &before) {
        Ok(v) => v,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    };

    let mut enriched: Vec<serde_json::Value> = Vec::with_capacity(sessions.len());
    for s in sessions.iter().take(limit * 2) {
        if let Some(proj) = params.project.as_deref()
            && s.project_path.as_deref() != Some(proj)
        {
            continue;
        }
        let summary = db.get_summary(&s.id, "L2_session").ok().flatten();
        let mut obj = serde_json::json!({
            "id": s.id,
            "source": s.source,
            "project_path": s.project_path,
            "title": s.title,
            "message_count": s.message_count,
            "updated_at": s.updated_at,
        });
        if let Some(sum) = summary {
            obj["l2_summary"] = serde_json::json!({
                "title": sum.title,
                "summary": sum.summary,
                "topics": sum.topics,
                "decisions": sum.decisions,
            });
        }
        enriched.push(obj);
        if enriched.len() >= limit {
            break;
        }
    }

    Json(serde_json::json!({
        "range": { "after": after, "before": before },
        "total": enriched.len(),
        "sessions": enriched,
    }))
    .into_response()
}

fn normalize_date_bound(raw: &str, is_start: bool) -> String {
    if raw.contains('T') {
        return raw.to_string();
    }
    if is_start {
        format!("{}T00:00:00+00:00", raw)
    } else {
        format!("{}T23:59:59+00:00", raw)
    }
}

#[derive(Deserialize)]
pub struct McpLogBody {
    pub tool: String,
    pub latency_ms: u64,
    pub success: bool,
    pub error: Option<String>,
    pub args: Option<String>,
    pub result: Option<String>,
}

/// POST /mcp/log —— 记录 mcp_call_log 一行 + 自增 mcp_calls metric。
///
/// memex-mcp 在每次工具调用完成后调一次这个端点，把 latency / args / result
/// 异步沉淀到 db。daemon 失败时静默忽略：mcp 调用本身的语义不应受 telemetry
/// 影响。
pub async fn mcp_log(
    State(db): State<AppState>,
    Json(body): Json<McpLogBody>,
) -> impl IntoResponse {
    use memex_core::storage::mcp_call_log::truncate_payload;

    let args = body.args.map(truncate_payload);
    let result = body.result.map(truncate_payload);

    let _ = db.increment_metric(memex_core::storage::metrics::METRIC_MCP_CALLS);
    let _ = db.insert_mcp_call(
        &body.tool,
        body.latency_ms,
        body.success,
        body.error.as_deref(),
        args.as_deref(),
        result.as_deref(),
    );

    Json(serde_json::json!({ "ok": true }))
}

fn err_msg(msg: &str) -> serde_json::Value {
    serde_json::json!({ "error": msg })
}

#[derive(Deserialize, Default)]
pub struct IngestBody {
    /// 可选 adapter 过滤，对应 `memex ingest --adapter claude_code` 等。None = 跑全部已知 adapter。
    #[serde(default)]
    pub adapter: Option<String>,
}

/// POST /ingest —— 手动触发一次全量 ingest。
///
/// daemon 启动时已经跑了一次 bootstrap ingest（lib.rs），watcher 也在持续监听
/// 文件变化。这个端点存在的意义是给 `memex ingest` 一个 RPC 通道，让用户手动
/// 触发也能复用同一个 ingest 路径，避免出现 "CLI 跑出来的 db 跟 daemon 跑出来
/// 的不一致" 这种诡异问题。
///
/// 用 `spawn_blocking` 把同步的 ingest::run_ingest 调进 tokio thread pool，避免
/// 阻塞 axum runtime；其他端点（search / stats）该响应该响应。
pub async fn ingest(
    State(db): State<AppState>,
    body: Option<Json<IngestBody>>,
) -> impl IntoResponse {
    let adapter = body.and_then(|Json(b)| b.adapter);
    let memex = memex_core::memex_dir();
    let db = Arc::clone(&db);
    let join = tokio::task::spawn_blocking(move || {
        ingest::run_ingest(&db, &memex, adapter.as_deref())
    })
    .await;

    match join {
        Ok(Ok(result)) => Json(serde_json::json!({
            "messages_ingested": result.messages_ingested,
            "chunks_created": result.chunks_created,
        }))
        .into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("ingest task join error: {}", e) })),
        )
            .into_response(),
    }
}
