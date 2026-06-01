use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{info, warn};

use memex_core::config::MemexConfig;
use memex_core::ingest;
use memex_core::storage::db::Db;

const DEBOUNCE_SECS: u64 = 2;

pub fn adapter_watch_dirs(memex_dir: &Path) -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let config = MemexConfig::load(memex_dir).unwrap_or_default();
    let mut dirs = Vec::new();

    if config.adapters.claude_code {
        let p = home.join(".claude/projects");
        if p.exists() { dirs.push(p); }
    }
    if config.adapters.cursor {
        let p = home.join(".cursor/projects");
        if p.exists() { dirs.push(p); }
    }
    if config.adapters.codex {
        let p = home.join(".codex");
        if p.exists() { dirs.push(p); }
    }
    if config.adapters.opencode {
        let p = home.join(".opencode/sessions");
        if p.exists() { dirs.push(p); }
    }
    if config.adapters.aider {
        let p = home.join(".aider");
        if p.exists() { dirs.push(p); }
    }
    if config.adapters.continue_dev {
        let p = home.join(".continue");
        if p.exists() { dirs.push(p); }
    }
    if config.adapters.cline {
        let p = home.join(".cline");
        if p.exists() { dirs.push(p); }
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

    let watch_dirs = adapter_watch_dirs(&memex_dir);
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
