use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use memex_core::config::MemexConfig;
use memex_core::ingest;
use memex_core::storage::db::Db;
use memex_core::storage::notifications::KIND_INGEST_FAILED;

const DEBOUNCE_SECS: u64 = 2;

pub fn adapter_watch_dirs(memex_dir: &Path) -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    let config = MemexConfig::load(memex_dir).unwrap_or_default();
    let mut dirs = Vec::new();

    if config.adapters.claude_code {
        let p = home.join(".claude/projects");
        if p.exists() {
            dirs.push(p);
        }
    }
    if config.adapters.cursor {
        let p = home.join(".cursor/projects");
        if p.exists() {
            dirs.push(p);
        }
    }
    if config.adapters.codex {
        let p = home.join(".codex");
        if p.exists() {
            dirs.push(p);
        }
    }
    if config.adapters.opencode {
        let p = home.join(".opencode/sessions");
        if p.exists() {
            dirs.push(p);
        }
    }
    if config.adapters.aider {
        let p = home.join(".aider");
        if p.exists() {
            dirs.push(p);
        }
    }
    if config.adapters.continue_dev {
        let p = home.join(".continue");
        if p.exists() {
            dirs.push(p);
        }
    }
    if config.adapters.cline {
        let p = home.join(".cline");
        if p.exists() {
            dirs.push(p);
        }
    }

    dirs
}

/// 启动 fsevent watcher + 后台 ingest 触发任务。
///
/// 返回 `Option<JoinHandle<()>>`：
/// - `Some(handle)` —— watcher task 在跑，调用方**必须**在 daemon shutdown
///   时 `handle.abort()`，否则 task 会被旧 `RecommendedWatcher` 卡住永远 pending
///   （watcher 闭包内的 `mpsc::Sender` clone 跟 watcher 一起被 task 持有，
///   `rx.recv()` 永远等不到 channel close）。watcher 不释放 → fsevent stream
///   不释放 → kqueue/fd 累积。多次 `daemon_restart` 后 fd 表撑爆，外层 spawn
///   `memex-cli` 子进程时 `pipe2()` / `posix_spawn` 会失败成 `EBADF`，UI
///   表现为 Connect 页 IDE 集成卡片永久 0/0，重启 app 才能恢复。
/// - `None` —— 没有 adapter 目录可监听（watcher 没启动）。
pub async fn start_watcher(
    db: Arc<Db>,
    memex_dir: PathBuf,
) -> Result<Option<JoinHandle<()>>> {
    let (tx, mut rx) = mpsc::channel::<()>(16);

    let watcher_tx = tx.clone();
    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res
                && matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
            {
                let dominated = event.paths.iter().any(|p| {
                    p.extension()
                        .is_some_and(|ext| ext == "jsonl" || ext == "json")
                });
                if dominated {
                    let _ = watcher_tx.blocking_send(());
                }
            }
        })?;

    let watch_dirs = adapter_watch_dirs(&memex_dir);
    if watch_dirs.is_empty() {
        info!("no adapter directories found to watch");
        return Ok(None);
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

    info!(
        "file watcher started, monitoring {} directories",
        watched.len()
    );

    let handle = tokio::spawn(async move {
        // `_keep` 让 watcher（持有 fsevent fd）的生命周期跟 task 绑定。
        // task 被外层 abort 时，drop 链：task → _keep → RecommendedWatcher
        // → fsevent stream close → fd 归还。
        let _keep = watcher;
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
                Err(e) => {
                    warn!("auto-ingest failed: {}", e);
                    // 用户没法主动看到 watcher 的静默失败 —— 写一条通知，UI Bell badge
                    // 会自动提示。通知写入失败时仍然继续（payload 序列化 + db.insert
                    // 都 fallible，但不能让通知层影响主流程）。
                    // 但是要尊重用户在 Settings 里的开关：关掉就静音。
                    if db.notification_enabled(KIND_INGEST_FAILED) {
                        let payload = serde_json::json!({
                            "error": e.to_string(),
                            "trigger": "watcher",
                        })
                        .to_string();
                        let _ = db.insert_notification(
                            KIND_INGEST_FAILED,
                            "采集源同步失败",
                            &format!("自动 ingest 失败：{}", e),
                            Some(&payload),
                        );
                    }
                }
            }
        }
    });

    Ok(Some(handle))
}
