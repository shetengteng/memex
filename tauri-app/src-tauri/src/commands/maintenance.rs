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

/// 彻底重置：停掉 daemon → 清空整个 memex 目录 → 重建目录 → 重启 daemon。
#[tauri::command]
pub async fn system_reset_all(
    app: AppHandle,
    state: tauri::State<'_, DaemonState>,
) -> CmdResult<SystemResetResult> {
    state.shutdown().await;

    let memex = memex_dir();
    let report = tokio::task::spawn_blocking(move || {
        let r = reset_all(&memex)?;
        memex_core::config::ensure_memex_dir(&memex)
            .map_err(|e| anyhow::anyhow!("failed to recreate memex dir: {e}"))?;
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
