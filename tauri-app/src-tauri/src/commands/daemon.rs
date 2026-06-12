//! Daemon 相关 IPC handler。
//!
//! Phase 4 起 daemon 完全跑在 Tauri 主进程内（`services/daemon.rs`），
//! 没有独立子进程，也没有"kill daemon"的概念。这里只保留：
//! * [`daemon_status`]：返回 in-process daemon 当前状态（pid / port / HTTP 健康）
//! * [`daemon_restart`]：shutdown 当前 task → spawn 新 task → 轮询 HTTP 就绪
//! * [`daemon_log_path`]：返回 stdout 日志路径（in-process 模式下文件可能不存在）
//! * [`daemon_restart_inner`]：给 `backup::import_db` / `maintenance::*` 复用
//!
//! 之前残留的 `stop_daemon_blocking` / `LockInfo` / `read_lock` 等已删除——
//! lock 文件由 `services/daemon.rs::spawn_in_process / DaemonHandle::shutdown`
//! 集中管理。

use std::process::Command;

use memex_core::memex_dir;
use serde::Serialize;

use super::error::CmdResult;
use crate::services::daemon::DaemonState;

#[derive(Debug, Clone, Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub http_ok: bool,
    pub started_at: Option<String>,
}

/// 异步探活：用 `tokio::task::spawn_blocking` 把同步 curl 调用挪到 blocking 线程池，
/// 不再阻塞 tokio worker。原来在 axum/Tauri 共享 runtime 上同步 `Command::output()`
/// 会卡住一整个 worker 最多 2 秒，连带影响其他 IPC 与 daemon 自身的 HTTP 处理。
async fn http_health_ok(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    let result = tokio::task::spawn_blocking(move || {
        Command::new("curl")
            .args([
                "-s",
                "-o",
                "/dev/null",
                "-w",
                "%{http_code}",
                "--max-time",
                "2",
                &url,
            ])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim() == "200")
            .unwrap_or(false)
    })
    .await;
    result.unwrap_or(false)
}

/// 返回 daemon 写 stdout 日志的绝对路径，方便 GUI 直接 `open` 这个文件。
///
/// Phase 4 后 daemon 跑在 Tauri 主进程里，没有独立 stdout 日志文件。这里返回
/// 的路径有可能并不存在，前端会优雅降级到"日志面板"或者 hide 按钮。保留 API
/// 仅为不打破前端 invoke 契约。
#[tauri::command]
pub fn daemon_log_path() -> String {
    memex_dir()
        .join("daemon.stdout.log")
        .to_string_lossy()
        .into_owned()
}

/// 用 in-process state 当唯一真相源生成 status：
/// * `state.snapshot() == Some` → running=true，再调 HTTP `/health` 探活，
///   未就绪时 running 仍是 true，前端按 `http_ok` 区分"启动中" vs "就绪"
/// * `state.snapshot() == None`  → 还没起 / 启动失败 → running=false
async fn build_status_from_state(state: &DaemonState) -> DaemonStatus {
    let snapshot = match state.snapshot().await {
        Some(s) => s,
        None => {
            return DaemonStatus {
                running: false,
                pid: None,
                port: None,
                http_ok: false,
                started_at: None,
            };
        }
    };
    let http_ok = http_health_ok(snapshot.port).await;
    DaemonStatus {
        running: true,
        pid: Some(snapshot.pid),
        port: Some(snapshot.port),
        http_ok,
        started_at: Some(snapshot.started_at),
    }
}

#[tauri::command]
pub async fn daemon_status(state: tauri::State<'_, DaemonState>) -> CmdResult<DaemonStatus> {
    Ok(build_status_from_state(&state).await)
}

#[tauri::command]
pub async fn daemon_restart(state: tauri::State<'_, DaemonState>) -> CmdResult<DaemonStatus> {
    daemon_restart_inner(&state).await
}

/// 同 crate 内部复用的重启入口。
///
/// `daemon_restart` IPC、`backup::import_db`、`maintenance::restart_after_reset`
/// 三处都需要"停掉当前 daemon → 起新 daemon → 等 HTTP 就绪"这同一段流程，
/// 提取这个函数避免逻辑漂移。
///
/// `?` 把 `state.restart` 的 anyhow::Error 自动转 `CmdError::Backend`，保留完整 context chain。
pub(crate) async fn daemon_restart_inner(state: &DaemonState) -> CmdResult<DaemonStatus> {
    state.restart().await?;

    // axum::serve 是异步起的，restart 返回时 HTTP 未必立刻可用。轮询 20 次，
    // 每次 150ms，总共最多等 3s；命中就立即返回，超时也返回当前 state 的
    // 快照，让前端区分"启动中"和"未启动"。
    //
    // 注意：必须用 `tokio::time::sleep` 而不是 `std::thread::sleep`。后者会
    // 阻塞整个 tokio worker 线程，把 axum 的其他请求和后续 IPC 都拖慢；in-process
    // daemon 跟所有 IPC 共享同一 runtime，3s 的同步 sleep 会让用户感觉「点了没反应」。
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let status = build_status_from_state(state).await;
        if status.http_ok {
            return Ok(status);
        }
    }
    Ok(build_status_from_state(state).await)
}
