use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{info, warn};

use memex_core::ingest;
use memex_core::storage::db::Db;

const DEBOUNCE_SECS: u64 = 2;

pub fn adapter_watch_dirs() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut dirs = Vec::new();

    let claude = home.join(".claude/projects");
    if claude.exists() {
        dirs.push(claude);
    }

    let cursor = home.join(".cursor/projects");
    if cursor.exists() {
        dirs.push(cursor);
    }

    let codex = home.join(".codex");
    if codex.exists() {
        dirs.push(codex);
    }

    let opencode = home.join(".opencode/sessions");
    if opencode.exists() {
        dirs.push(opencode);
    }

    dirs
}

pub async fn start_watcher(db: Arc<Db>, memex_dir: PathBuf) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<()>(16);

    let watcher_tx = tx.clone();
    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_)
                ) {
                    let dominated = event.paths.iter().any(|p| {
                        p.extension().is_some_and(|ext| ext == "jsonl" || ext == "json")
                    });
                    if dominated {
                        let _ = watcher_tx.blocking_send(());
                    }
                }
            }
        })?;

    let watch_dirs = adapter_watch_dirs();
    if watch_dirs.is_empty() {
        info!("no adapter directories found to watch");
        return Ok(());
    }

    let mut watched = HashSet::new();
    for dir in &watch_dirs {
        if watcher.watch(dir, RecursiveMode::Recursive).is_ok() {
            watched.insert(dir.clone());
            info!("watching: {}", dir.display());
        } else {
            warn!("failed to watch: {}", dir.display());
        }
    }

    info!("file watcher started, monitoring {} directories", watched.len());

    let _watcher = watcher;
    tokio::spawn(async move {
        let _keep = _watcher;
        loop {
            if rx.recv().await.is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(DEBOUNCE_SECS)).await;
            while rx.try_recv().is_ok() {}

            info!("file change detected, running ingest...");
            match ingest::run_ingest(&db, &memex_dir, None) {
                Ok(r) => {
                    if r.messages_ingested > 0 {
                        info!(
                            "auto-ingest: {} messages, {} chunks",
                            r.messages_ingested, r.chunks_created
                        );
                    }
                }
                Err(e) => warn!("auto-ingest failed: {}", e),
            }
        }
    });

    Ok(())
}
