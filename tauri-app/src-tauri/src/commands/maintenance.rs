//! 维护操作的 Tauri 入口：重建索引、彻底重置。
//!
//! 顺序：
//!   1. 先停 daemon（它持有 `memex.db` 的写句柄；不停掉直接删 db
//!      在 macOS/Linux 上虽然不会立即报错，但残留的 WAL 会让重建脏掉）。
//!   2. 调 `memex_core::maintenance` 做文件系统删除。
//!   3. 异步触发 app 退出，让用户手动重启进入干净状态。
//!
//! 调度选项：
//! - 返回 `ResetReport` 给前端展示「删除了 N 个文件、约 M MB」。
//! - `exit_app` 由后台任务延迟 600ms 执行，给 IPC 响应留时间。

use std::process::Command;
use std::time::Duration;

use memex_core::maintenance::{ResetReport, reset_all, reset_index_only};
use memex_core::memex_dir;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::daemon::{daemon_status, read_lock_for_maintenance, is_process_alive_for_maintenance};

#[derive(Debug, Clone, Serialize)]
pub struct SystemResetResult {
    pub mode: &'static str,
    pub report: ResetReport,
}

/// 重建索引：停掉 daemon → 删 `memex.db*` → 退出 app。
/// 保留 `sessions/*.md` / `config.toml` / `redactions.yaml`。
/// 重启后 daemon 会重新跑 ingest，但 LLM 摘要需要用户重新触发。
#[tauri::command]
pub async fn system_reset_index(app: AppHandle) -> Result<SystemResetResult, String> {
    stop_daemon_blocking();

    let memex = memex_dir();
    let report = tokio::task::spawn_blocking(move || reset_index_only(&memex))
        .await
        .map_err(|e| format!("join error: {e}"))?
        .map_err(|e| e.to_string())?;

    schedule_exit(app);

    Ok(SystemResetResult {
        mode: "index",
        report,
    })
}

/// 彻底重置：停掉 daemon → 清空整个 memex 目录 → 退出 app。
#[tauri::command]
pub async fn system_reset_all(app: AppHandle) -> Result<SystemResetResult, String> {
    stop_daemon_blocking();

    let memex = memex_dir();
    let report = tokio::task::spawn_blocking(move || reset_all(&memex))
        .await
        .map_err(|e| format!("join error: {e}"))?
        .map_err(|e| e.to_string())?;

    schedule_exit(app);

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

    let _ = Command::new("kill")
        .args(["-TERM", &info.pid.to_string()])
        .status();
    std::thread::sleep(Duration::from_millis(800));
    if is_process_alive_for_maintenance(info.pid) {
        let _ = Command::new("kill")
            .args(["-KILL", &info.pid.to_string()])
            .status();
        std::thread::sleep(Duration::from_millis(200));
    }

    let _ = std::fs::remove_file(memex_dir().join("daemon.lock"));
}

/// 延迟 600ms 退出 app，给前端 IPC 响应留时间。
fn schedule_exit(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        // 先把所有窗口收起来，给用户一个"操作生效"的视觉反馈。
        for (_label, win) in app.webview_windows() {
            let _ = win.hide();
        }
        app.exit(0);
    });
    // 让 `daemon_status` 缓存里的旧状态被清掉（best-effort，不阻塞主路径）。
    tauri::async_runtime::spawn(async move {
        let _ = daemon_status().await;
    });
}
