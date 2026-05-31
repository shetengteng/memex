pub mod launchd;
pub mod lockfile;
pub mod routes;
pub mod watcher;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::info;

use memex_core::config::ensure_memex_dir;
use memex_core::storage::db::Db;

pub const DEFAULT_PORT: u16 = 9999;

pub async fn run(port: u16) -> Result<()> {
    let memex_dir = memex_core::memex_dir();
    ensure_memex_dir(&memex_dir)?;

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
    info!("daemon.lock written (pid={}, port={})", std::process::id(), port);

    let watcher_db = Arc::clone(&db);
    let watcher_dir = memex_dir.clone();
    watcher::start_watcher(watcher_db, watcher_dir).await?;

    let app = build_router(Arc::clone(&db));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    info!("HTTP server listening on http://{}", addr);

    let lock_dir = memex_dir.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    lockfile::remove_lock(&lock_dir);
    info!("daemon stopped");
    Ok(())
}

pub fn build_router(db: Arc<Db>) -> Router {
    Router::new()
        .route("/health", get(routes::health))
        .route("/search", get(routes::search))
        .route("/sessions", get(routes::list_sessions))
        .route("/sessions/{id}", get(routes::get_session))
        .route("/stats", get(routes::stats))
        .route("/config", get(routes::get_config).post(routes::set_config))
        .with_state(db)
}

async fn shutdown_signal() {
    let ctrl_c = async { signal::ctrl_c().await.unwrap_or(()) };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
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
