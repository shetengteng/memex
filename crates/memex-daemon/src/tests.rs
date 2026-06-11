//! HTTP 路由集成测试。我们用 `tower::oneshot` 跑 router，
//! 这样不用真的绑端口就能跑通整个 handler 链路。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use memex_core::storage::db::Db;
use memex_core::storage::models::{Chunk, ChunkMetadata, ChunkType};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use crate::lockfile::{read_lock, remove_lock, write_lock};

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
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response.into_body()).await;
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_stats_zero_for_empty_db() {
    let response = empty_db_router()
        .oneshot(
            Request::builder()
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response.into_body()).await;
    assert_eq!(json["sessions"], 0);
    assert_eq!(json["messages"], 0);
    assert_eq!(json["chunks"], 0);
    assert_eq!(json["sources"], 0);
    // Phase 5b-2: today / last_7_days 永远是对象（空 db 时是空对象），CLI
    // 端解析时不允许它们是 null。
    assert!(json["today"].is_object(), "today must be object");
    assert!(
        json["last_7_days"].is_object(),
        "last_7_days must be object"
    );
    assert!(json["today"].as_object().unwrap().is_empty());
    assert!(json["last_7_days"].as_object().unwrap().is_empty());
}

/// POST /ingest 在空 ~/.memex 目录下应当跑成功（0 messages 0 chunks），不能 5xx。
/// 由于 ingest 内部读 `memex_core::memex_dir()`，单测必须把 HOME 重定向到 tempdir
/// 才能避免污染开发者真实 db。
#[tokio::test]
async fn test_ingest_returns_zero_on_empty_home() {
    let tmp = tempfile::TempDir::new().unwrap();
    let prev_home = std::env::var_os("HOME");
    // SAFETY: tests 默认串行，且只改 HOME，不会污染并发线程。
    unsafe {
        std::env::set_var("HOME", tmp.path());
    }

    let db = Arc::new(Db::open_in_memory().unwrap());
    let app = crate::build_router(db);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ingest")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    // 还原 HOME，无论后续 assert 是否失败都不能漏。
    unsafe {
        if let Some(v) = prev_home {
            std::env::set_var("HOME", v);
        } else {
            std::env::remove_var("HOME");
        }
    }

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response.into_body()).await;
    assert_eq!(json["messages_ingested"], 0);
    assert_eq!(json["chunks_created"], 0);
}

#[tokio::test]
async fn test_stats_aggregates_today_and_last_7_days() {
    // 准备：插入两条 search 记录 + 一条 ingest 记录 + 一条 mcp 记录，
    // 所有计数都打在"今天"。今日和 7 日累计的值应当一致。
    let db = Arc::new(Db::open_in_memory().unwrap());
    db.record_search_latency(120).unwrap();
    db.record_search_latency(300).unwrap();
    db.increment_metric_by("ingest_messages", 42).unwrap();
    db.increment_metric("mcp_calls").unwrap();

    let app = crate::build_router(db);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response.into_body()).await;

    // record_search_latency 既写 search_count 也写 search_latency_ms_total，但
    // 我们只校验 search_count，避免 latency-related 列名变动时打破测试。
    assert_eq!(json["today"]["search_count"], 2);
    assert_eq!(json["today"]["mcp_calls"], 1);
    assert_eq!(json["today"]["ingest_messages"], 42);
    // 既然全部 metric 都在今天，那 last_7_days 至少等于今天的值。
    assert_eq!(json["last_7_days"]["search_count"], 2);
    assert_eq!(json["last_7_days"]["mcp_calls"], 1);
    assert_eq!(json["last_7_days"]["ingest_messages"], 42);
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
    db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl", 0, 0)
        .unwrap();
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
    db.insert_session("s1", "claude_code", Some("/proj"), "/f.jsonl", 0, 0)
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

/// Phase 1 新增：验证 `run_in_process` 的核心契约 —— 启动后持续运行直到 caller
/// 通过 `shutdown` future 触发退出，且不会自己写 lockfile（同进程内只跑一份）。
///
/// 这是把 daemon 嵌进 Tauri 主进程的基础：caller 必须能完全控制生命周期。
/// 用 port=0 让 OS 动态分配端口，避免和真实 daemon (9999) 冲突。
#[tokio::test]
async fn test_run_in_process_obeys_external_shutdown() {
    let tmp = TempDir::new().unwrap();
    let memex_dir = tmp.path().to_path_buf();
    let db = Arc::new(Db::open_in_memory().unwrap());

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown = async move {
        let _ = shutdown_rx.await;
    };

    let server = tokio::spawn(crate::run_in_process(memex_dir, db, 0, shutdown));

    // 给 axum / watcher / spawn_blocking 启动时间。100ms 在 macOS CI 下足够。
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(
        !server.is_finished(),
        "run_in_process must keep running until shutdown trigger",
    );

    // run_in_process 必须**不**写 lockfile —— in-process 模式同进程内只有一个
    // 实例，文件锁没有意义；之所以现在能保留是因为 binary `run()` 才会写。
    assert!(
        !tmp.path().join("daemon.lock").exists(),
        "run_in_process must not write daemon.lock",
    );

    let _ = shutdown_tx.send(());

    let result = tokio::time::timeout(std::time::Duration::from_secs(3), server)
        .await
        .expect("server should shut down within 3s after trigger")
        .expect("server task panicked");
    assert!(
        result.is_ok(),
        "run_in_process should return Ok on graceful shutdown, got {:?}",
        result.err(),
    );
}
