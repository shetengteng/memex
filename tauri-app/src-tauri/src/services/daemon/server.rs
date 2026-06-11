//! axum HTTP server 入口 + 路由表。
//!
//! Phase 6 起，daemon 的代码物理上沉到 `crate::services::daemon::*`。
//! 这里负责 wire 起 watcher + bootstrap ingest + axum 三件套，跑在
//! Tauri 主进程的 tokio runtime 上。
//!
//! 唯一对外（crate 内）入口 = [`run_in_process`]，由 [`super::handle::spawn_in_process`]
//! 调起。

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tracing::{info, warn};

use memex_core::ingest;
use memex_core::storage::db::Db;

use super::{routes, watcher, web};

pub const DEFAULT_PORT: u16 = 9999;

/// In-process 入口：caller 已 open db、已 ensure memex_dir、已设置 logger。
///
/// **lifecycle 期望**（与早期 standalone binary 模式的差异）：
/// * 不写 / 不读 `daemon.lock`（lockfile 由 caller 在调用前后管理，本函数只跑 axum + watcher）
/// * 不安装 SIGTERM/Ctrl-C handler —— 让 caller 通过 `shutdown` future 决定
///   生命周期，避免 daemon 跟 Tauri 主进程抢同一个 signal stream
/// * 不重置 tracing subscriber —— caller 已经有 logger 配置
pub async fn run_in_process<F>(
    memex_dir: PathBuf,
    db: Arc<Db>,
    port: u16,
    shutdown: F,
) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let watcher_db = Arc::clone(&db);
    let watcher_dir = memex_dir.clone();
    watcher::start_watcher(watcher_db, watcher_dir).await?;

    // 启动时主动跑一次全量 ingest。
    // file watcher 只能监听 .jsonl/.json 后缀，但 Cursor 走 SQLite KV
    // (`state.vscdb`)，watcher 永远抓不到它的变化。如果不在这里主动 ingest 一次，
    // 重装 / 首启 memex 后，用户必须手动 `memex ingest` 才能看到 cursor 数据。
    //
    // 用 spawn_blocking 异步跑，不阻塞 daemon 起 HTTP 服务。
    let bootstrap_db = Arc::clone(&db);
    let bootstrap_dir = memex_dir.clone();
    tokio::task::spawn_blocking(move || {
        info!("daemon: bootstrap ingest starting");
        match ingest::run_ingest(&bootstrap_db, &bootstrap_dir, None) {
            Ok(r) => info!(
                "daemon: bootstrap ingest done ({} messages, {} chunks)",
                r.messages_ingested, r.chunks_created
            ),
            Err(e) => warn!("daemon: bootstrap ingest failed: {}", e),
        }
    });

    let app = build_router(Arc::clone(&db));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    info!("daemon HTTP server listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    info!("daemon HTTP server stopped");
    Ok(())
}

pub fn build_router(db: Arc<Db>) -> Router {
    Router::new()
        .route("/health", get(routes::health))
        .route("/search", get(routes::search))
        .route("/sessions", get(routes::list_sessions))
        .route("/sessions/{id}", get(routes::get_session))
        .route("/stats", get(routes::stats))
        .route("/stats/breakdown", get(routes::stats_breakdown))
        .route("/timeline", get(routes::timeline))
        .route("/ingest", post(routes::ingest))
        .route("/context", get(routes::context))
        .route("/sessions/range", get(routes::sessions_range))
        .route("/mcp/log", post(routes::mcp_log))
        .route("/config", get(routes::get_config).post(routes::set_config))
        .route("/summaries/stats", get(routes::summary_stats))
        .route("/sessions/{id}/summary", get(routes::get_session_summary))
        .with_state(db)
        .merge(web::static_router())
}
