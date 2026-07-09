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

/// 判断一次 fs 事件是否指向真正的 session 文件。
///
/// 旧过滤器是 `ext == "jsonl" || ext == "json"`，但 Cursor 会在
/// `~/.cursor/projects/<workspace>/mcp-cache.json` 里缓存 MCP 调用元数据，
/// **每次 MCP 调用都会重写**这个文件 —— watcher 见到 modify 事件就触发一次
/// ingest，daemon 因此每 1-2 分钟就跑一遍完整扫描，把 DB / WAL 锁住。
///
/// 会话文件的实际约定：
/// - Claude Code / Cursor / Codex / opencode：`.jsonl`（一行一 event）
/// - Kiro：`.json`，但路径必然包含 `workspace-sessions` 段
///
/// 其它任何 `.json` / `.md` / `.yaml`（IDE 配置、缓存、user 项目文档）都不该
/// 触发 ingest —— 收集器每次扫源目录时会重新按扩展名 + 内容识别，watcher
/// 只是「粗触发」。
fn is_session_file(p: &Path) -> bool {
    let Some(ext) = p.extension() else {
        return false;
    };
    if ext == "jsonl" {
        return true;
    }
    if ext == "json" && p.to_string_lossy().contains("workspace-sessions") {
        return true;
    }
    false
}

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
    if config.adapters.kiro {
        // macOS 唯一路径：Kiro 只在 macOS 有官方发行
        let p = home.join(
            "Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/workspace-sessions",
        );
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
                let dominated = event.paths.iter().any(|p| is_session_file(p));
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
                    // ingest 完做一次 WAL checkpoint，把 -wal 收敛回 0。
                    // 达不到的（有其它连接持锁）忽略，下一轮再试。
                    if let Err(e) = db.wal_checkpoint_truncate() {
                        warn!("wal checkpoint after ingest failed: {}", e);
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

#[cfg(test)]
mod tests {
    use super::is_session_file;
    use std::path::PathBuf;

    #[test]
    fn accepts_jsonl_sessions() {
        let p = PathBuf::from("/x/.claude/projects/foo/bar.jsonl");
        assert!(is_session_file(&p));
    }

    #[test]
    fn accepts_kiro_workspace_sessions_json() {
        let p = PathBuf::from(
            "/x/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/workspace-sessions/abc.json",
        );
        assert!(is_session_file(&p));
    }

    /// Cursor 的 mcp-cache.json 是频繁重写的配置缓存，不能触发 ingest —
    /// 这是 daemon 每 1-2 分钟空转的根源。
    #[test]
    fn rejects_cursor_mcp_cache_json() {
        let p = PathBuf::from("/x/.cursor/projects/Users-Foo/mcp-cache.json");
        assert!(!is_session_file(&p));
    }

    #[test]
    fn rejects_non_session_extensions() {
        for name in ["a.md", "a.yaml", "a.png", "a.txt", "a"] {
            let p = PathBuf::from(format!("/x/{name}"));
            assert!(!is_session_file(&p), "{name} should be ignored");
        }
    }
}
