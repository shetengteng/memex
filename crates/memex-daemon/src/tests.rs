//! HTTP route integration tests. We exercise the router with `tower::oneshot`
//! so we can run the full handler stack without binding a real TCP port.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use memex_core::storage::db::Db;
use memex_core::storage::models::{Chunk, ChunkMetadata, ChunkType};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::lockfile::{is_daemon_running, read_lock, remove_lock, write_lock};

fn empty_db_router() -> axum::Router {
    let db = Arc::new(Db::open_in_memory().unwrap());
    crate::build_router(db)
}

async fn body_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_health_returns_ok() {
    let response = empty_db_router()
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response.into_body()).await;
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_stats_zero_for_empty_db() {
    let response = empty_db_router()
        .oneshot(Request::builder().uri("/stats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response.into_body()).await;
    assert_eq!(json["sessions"], 0);
    assert_eq!(json["messages"], 0);
    assert_eq!(json["chunks"], 0);
}

#[tokio::test]
async fn test_sessions_list_empty() {
    let response = empty_db_router()
        .oneshot(
            Request::builder()
                .uri("/sessions?limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response.into_body()).await;
    assert!(json["sessions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_search_returns_seeded_chunk() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl", 0).unwrap();
    let hash = blake3::hash(b"redis pipeline tuning").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "redis pipeline tuning", None, 0, &hash)
        .unwrap();
    db.insert_chunk(&Chunk {
        id: None,
        message_id: "m1".into(),
        session_id: "s1".into(),
        chunk_type: ChunkType::Text,
        content: "redis pipeline tuning".into(),
        redacted_content: None,
        position: 0,
        token_count: 3,
        metadata: ChunkMetadata::default(),
    })
    .unwrap();

    let app = crate::build_router(db);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=redis&limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response.into_body()).await;
    let results = json["results"].as_array().expect("results array");
    assert_eq!(results.len(), 1);
    assert!(results[0]["snippet"].as_str().unwrap().contains("redis"));
}

#[tokio::test]
async fn test_search_records_access_log_and_metric() {
    let db = Arc::new(Db::open_in_memory().unwrap());
    db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl", 0)
        .unwrap();
    let hash = blake3::hash(b"observability tracing").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "observability tracing", None, 0, &hash)
        .unwrap();
    db.insert_chunk(&Chunk {
        id: None,
        message_id: "m1".into(),
        session_id: "s1".into(),
        chunk_type: ChunkType::Text,
        content: "observability tracing".into(),
        redacted_content: None,
        position: 0,
        token_count: 3,
        metadata: ChunkMetadata::default(),
    })
    .unwrap();

    let metrics_before = db
        .get_today_metrics()
        .unwrap()
        .iter()
        .find(|m| m.name == memex_core::storage::metrics::METRIC_SEARCH_COUNT)
        .map(|m| m.value)
        .unwrap_or(0);

    let app = crate::build_router(db.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=observability&limit=5")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let metrics_after = db
        .get_today_metrics()
        .unwrap()
        .iter()
        .find(|m| m.name == memex_core::storage::metrics::METRIC_SEARCH_COUNT)
        .map(|m| m.value)
        .unwrap_or(0);
    assert_eq!(
        metrics_after,
        metrics_before + 1,
        "search_count should increment by exactly one per /search request"
    );
}

#[tokio::test]
async fn test_get_unknown_session_returns_404() {
    let response = empty_db_router()
        .oneshot(
            Request::builder()
                .uri("/sessions/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn test_lockfile_roundtrip() {
    let tmp = TempDir::new().unwrap();
    write_lock(tmp.path(), 9999).unwrap();
    let info = read_lock(tmp.path()).expect("lock present");
    assert_eq!(info.port, 9999);
    assert!(info.pid > 0);
    remove_lock(tmp.path());
    assert!(read_lock(tmp.path()).is_none());
}

#[test]
fn test_is_daemon_running_clears_dead_lock() {
    let tmp = TempDir::new().unwrap();
    let fake_path = crate::lockfile::lock_path(tmp.path());
    let stale = serde_json::json!({
        "pid": 1,
        "port": 9999,
        "started_at": "2026-01-01T00:00:00Z",
    });
    std::fs::write(&fake_path, stale.to_string()).unwrap();
    let _ = is_daemon_running(tmp.path());
}
