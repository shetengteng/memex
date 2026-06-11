//! 维护操作的 Tauri 入口：重建索引、彻底重置。
//!
//! 顺序：
//!   1. 先停 daemon（它持有 `memex.db` 的写句柄；不停掉直接删 db
//!      在 macOS/Linux 上虽然不会立即报错，但残留的 WAL 会让重建脏掉）。
//!   2. 调 `memex_core::maintenance` 做文件系统删除。
//!   3. 重启 daemon 并通知前端刷新，app 保持运行。

use std::process::Command;
use std::time::Duration;

use memex_core::maintenance::{ResetReport, reset_all, reset_index_only};
use memex_core::memex_dir;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::daemon::{
    daemon_restart_inner, is_process_alive_for_maintenance, read_lock_for_maintenance,
};
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
pub async fn system_reset_index(app: AppHandle) -> CmdResult<SystemResetResult> {
    stop_daemon_blocking();

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
pub async fn system_reset_all(app: AppHandle) -> CmdResult<SystemResetResult> {
    stop_daemon_blocking();

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

/// 同步杀掉本机 daemon 进程并清理 lock。daemon.rs 里已有 daemon_restart 用到了
/// 同样的逻辑，但那个函数还会顺带拉起新进程；这里只杀不拉。
///
/// 等同 daemon.rs 的"杀 daemon"片段，但抽出来供 reset 路径复用，
/// 避免互相 import 引起循环依赖。
fn stop_daemon_blocking() {
    let Some(info) = read_lock_for_maintenance() else {
        return;
    };
    if !is_process_alive_for_maintenance(info.pid) {
        let _ = std::fs::remove_file(memex_dir().join("daemon.lock"));
        return;
    }

    if let Err(e) = Command::new("kill")
        .args(["-TERM", &info.pid.to_string()])
        .status()
    {
        tracing::warn!(pid = info.pid, error = %e, "failed to send SIGTERM to daemon");
    }
    std::thread::sleep(Duration::from_millis(800));
    if is_process_alive_for_maintenance(info.pid) {
        if let Err(e) = Command::new("kill")
            .args(["-KILL", &info.pid.to_string()])
            .status()
        {
            tracing::warn!(pid = info.pid, error = %e, "failed to send SIGKILL to daemon");
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    let _ = std::fs::remove_file(memex_dir().join("daemon.lock"));
}

/// 重置后重启 daemon 并通知前端刷新，不再退出 app。
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
