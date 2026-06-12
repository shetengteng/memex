//! 维护操作的 Tauri 入口：重建索引、彻底重置。
//!
//! 顺序：
//!   1. 先 `state.shutdown()` 停掉 in-process daemon（释放 `Arc<Db>` 上所有
//!      引用，让 SQLite WAL 把 memex.db 解锁），lock 文件也由 shutdown 内部
//!      负责删除
//!   2. 调 `memex_core::maintenance` 做文件系统删除
//!   3. 异步 `daemon_restart_inner` 把 daemon 拉回来，并通过 `reset-complete`
//!      事件通知前端刷新；app 保持运行

use std::time::Duration;

use memex_core::maintenance::{ResetReport, reset_all, reset_index_only};
use memex_core::memex_dir;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::daemon::daemon_restart_inner;
use super::error::{CmdError, CmdResult};
use crate::services::daemon::DaemonState;

#[derive(Debug, Clone, Serialize)]
pub struct SystemResetResult {
    pub mode: &'static str,
    pub report: ResetReport,
}

/// 重建索引：停掉 daemon → 删 `memex.db*` → 重启 daemon。
/// 保留 `sessions/*.md` / `config.toml` / `redactions.yaml`。
/// daemon 会重新跑 ingest，但 LLM 摘要需要用户重新触发。
#[tauri::command]
pub async fn system_reset_index(
    app: AppHandle,
    state: tauri::State<'_, DaemonState>,
) -> CmdResult<SystemResetResult> {
    state.shutdown().await;

    let memex = memex_dir();
    let report = tokio::task::spawn_blocking(move || reset_index_only(&memex))
        .await
        .map_err(|e| CmdError::Backend(format!("join error: {e}")))??;

    restart_after_reset(app);

    Ok(SystemResetResult {
        mode: "index",
        report,
    })
}

/// 彻底重置：停掉 daemon → 等 watcher/fs 余晖 → 清空整个 memex 目录 →
/// 重建目录骨架 + 校验空 db 能 open → 重启 daemon。
///
/// 这里的等待和 open-then-close 校验都是为了避开两个真实事故：
/// 1. macOS fsevent watcher 在 shutdown 后还会有一两次延迟写，导致
///    `fs::remove_dir_all(sessions)` 报 `Directory not empty (os error 66)`。
///    300ms grace sleep 让 watcher 释放 fd，再走 reset_all 内部 5 次带 backoff
///    的重试，基本能稳定删干净。
/// 2. 上一次执行如果在删到一半时崩了，会留下 0 字节或被 truncate 的 memex.db。
///    daemon 重启 open 这个文件直接报 SQLite I/O error 522 — 用户体验是
///    "清空之后整个 app 就坏了"。这里在 reset_all 之后立刻 open 一次新 db
///    跑 schema migrations，确保下次 daemon 启动绝对不会拿到半截 db。
#[tauri::command]
pub async fn system_reset_all(
    app: AppHandle,
    state: tauri::State<'_, DaemonState>,
) -> CmdResult<SystemResetResult> {
    state.shutdown().await;

    // 给 fsevent watcher / 后台 ingest task 一个释放 fd 的机会
    tokio::time::sleep(Duration::from_millis(300)).await;

    let memex = memex_dir();
    let report = tokio::task::spawn_blocking(move || {
        let r = reset_all(&memex)?;
        memex_core::config::ensure_memex_dir(&memex)
            .map_err(|e| anyhow::anyhow!("failed to recreate memex dir: {e}"))?;
        // 验证 fresh db 能起来 —— open() 内部会跑全部 schema migrations。
        // 失败的话说明文件系统状态还没干净（极少见，但抓得到就告警），
        // daemon 重启时也会再 open 一次，至此已经是 PRAGMA journal_mode=WAL
        // 的合法 SQLite 文件。
        let db_path = memex.join("memex.db");
        memex_core::storage::db::Db::open(&db_path)
            .map_err(|e| anyhow::anyhow!("post-reset db open failed: {e}"))?;
        Ok::<_, anyhow::Error>(r)
    })
    .await
    .map_err(|e| CmdError::Backend(format!("join error: {e}")))??;

    restart_after_reset(app);

    Ok(SystemResetResult {
        mode: "all",
        report,
    })
}

/// 重置后重启 daemon 并通知前端刷新，不再退出 app。
///
/// `daemon_restart_inner` 内部会通过 `DaemonState::restart`：
/// 1. shutdown 当前 task（这里其实没什么可 shutdown 的，前面已经 shutdown 过）
/// 2. spawn 新 task，重新 open db
/// 3. 轮询 HTTP /health 直到就绪
fn restart_after_reset(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let state = app.state::<DaemonState>();
        if let Err(e) = daemon_restart_inner(&state).await {
            tracing::warn!(error = %e, "failed to restart daemon after reset");
        }
        // app.emit failures here mean no subscribers — UI is free to refresh
        // on its own polling. Not worth logging on every reset.
        let _ = app.emit("reset-complete", ());
    });
}
