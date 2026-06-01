use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use memex_core::retriever::{Retriever, SearchFilter};
use memex_core::storage::db::Db;

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

pub async fn get_session(
    State(db): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match db.get_session_detail(&id) {
        Ok(Some(detail)) => Json(serde_json::json!(detail)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "session not found"
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(err_body(&e))).into_response(),
    }
}

#[derive(Serialize)]
struct StatsResponse {
    sessions: u64,
    messages: u64,
    chunks: u64,
    sources: u64,
}

pub async fn stats(State(db): State<AppState>) -> impl IntoResponse {
    let resp = StatsResponse {
        sessions: db.session_count().unwrap_or(0),
        messages: db.message_count().unwrap_or(0),
        chunks: db.chunk_count().unwrap_or(0),
        sources: db.source_count().unwrap_or(0),
    };
    Json(resp)
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
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({
            "error": "key not found"
        }))).into_response(),
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
                let day = grouped
                    .entry(e.date.clone())
                    .or_insert_with(|| serde_json::json!({
                        "date": e.date,
                        "sessions": 0i64,
                        "messages": 0i64,
                        "by_adapter": {}
                    }));
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
