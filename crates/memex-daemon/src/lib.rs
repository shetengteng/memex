//! Memex 后台守护进程 —— 提供 HTTP API（search / hooks / summary），
//! 周期性触发 ingest，监听 IDE 历史变动，向 menubar 推送通知。
//!
//! 这是 `memex-daemon` binary 的 library 入口；二进制 main.rs 只负责把
//! [`Args`] 解析后调到 [`run_with_args`]。

#![warn(rust_2018_idioms)]
#![warn(clippy::all)]

pub mod launchd;
pub mod lockfile;
pub mod logging;
pub mod routes;
pub mod watcher;
pub mod web;

#[cfg(test)]
mod tests;

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, warn};

use memex_core::config::ensure_memex_dir;
use memex_core::ingest;
use memex_core::storage::db::Db;

pub const DEFAULT_PORT: u16 = 9999;

/// Standalone binary 入口：负责整套生命周期 —— 文件日志、lockfile、SIGTERM/Ctrl-C
/// 处理，最终把工作下放给 [`run_in_process`]。仅 `memex-daemon` binary main.rs 调用。
pub async fn run(port: u16) -> Result<()> {
    let memex_dir = memex_core::memex_dir();
    ensure_memex_dir(&memex_dir)?;

    let _log_guard = logging::setup_file_logging(&memex_dir)?;
    logging::rotate_old_logs(&memex_dir);

    if let Some(existing) = lockfile::is_daemon_running(&memex_dir) {
        anyhow::bail!(
            "daemon already running (pid={}, port={})",
            existing.pid,
            existing.port
        );
    }

    let db_path = memex_dir.join("memex.db");
    let db = Arc::new(Db::open(&db_path)?);

    lockfile::write_lock(&memex_dir, port)?;
    info!(
        "daemon.lock written (pid={}, port={})",
        std::process::id(),
        port
    );

    let lock_dir = memex_dir.clone();
    let result = run_in_process(memex_dir, db, port, shutdown_signal()).await;

    // 即便 run_in_process 报错也清 lock，避免下次启动卡在 "already running"。
    lockfile::remove_lock(&lock_dir);
    info!("daemon stopped");
    result
}

/// In-process 入口：caller 已 open db、已设置 logger、已 ensure memex_dir。
///
/// 跟 [`run`] 的差异：
/// * 不写 / 不读 `daemon.lock`（同一进程内只会有一个实例，不需要文件锁去重）
/// * 不安装 SIGTERM/Ctrl-C handler —— 让 caller 通过 `shutdown` future 决定
///   生命周期，避免 daemon 跟 Tauri 主进程抢同一个 signal stream
/// * 不重置 tracing subscriber —— caller 已经有 logger 配置
///
/// 这条路径是后续合并到 Tauri 主进程的"标准接入点"：Tauri 启动时
/// `tauri::async_runtime::spawn(memex_daemon::run_in_process(...))`，
/// 退出时把 caller 持有的 shutdown trigger 取消即可。
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
        .route("/config", get(routes::get_config).post(routes::set_config))
        .route("/summaries/stats", get(routes::summary_stats))
        .route("/sessions/{id}/summary", get(routes::get_session_summary))
        .with_state(db)
        .merge(web::static_router())
}

async fn shutdown_signal() {
    let ctrl_c = async { signal::ctrl_c().await.unwrap_or(()) };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("INVARIANT: SIGTERM handler must install on Unix")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => info!("received Ctrl+C"),
        () = terminate => info!("received SIGTERM"),
    }
}
