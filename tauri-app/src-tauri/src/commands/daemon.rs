use std::process::{Command, Stdio};

use memex_core::memex_dir;
use serde::{Deserialize, Serialize};

use super::error::CmdResult;
use crate::services::daemon::DaemonState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub pid: u32,
    pub port: u16,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub http_ok: bool,
    pub started_at: Option<String>,
}

fn read_lock() -> Option<LockInfo> {
    let path = memex_dir().join("daemon.lock");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 暴露给 maintenance.rs 复用同样的杀 daemon 流程，避免在两个文件里重复
/// 实现 / 维护一份 lock 文件解析。仅对同 crate 可见。
pub(crate) fn read_lock_for_maintenance() -> Option<LockInfo> {
    read_lock()
}

/// 同上：暴露 `kill -0` 判活给 maintenance.rs。
pub(crate) fn is_process_alive_for_maintenance(pid: u32) -> bool {
    is_process_alive(pid)
}

fn is_process_alive(pid: u32) -> bool {
    // kill -0 在进程存在且我们有权限给它发信号时返回 0。
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 优雅地停止 daemon：先 SIGTERM 等 800ms，仍存活则 SIGKILL。最后清掉
/// 过期 lock 文件。Quit 流程、`daemon_restart` 都复用这条路径，确保单一事实源。
///
/// Phase 2 self-pid 守卫：从 Phase 2 起 in-process daemon 把主进程 PID 写进
/// lock，如果不加守卫这个函数会把 Tauri 主进程自己杀掉。pid==self 时只清 lock
/// 不发信号，真正的 in-process shutdown 走
/// [`crate::services::daemon::DaemonHandle::shutdown_blocking`]。
///
/// fire-and-forget：信号发了 + lock 清了即可，不阻塞等内核 reap zombie。
pub(crate) fn stop_daemon_blocking() {
    let Some(info) = read_lock() else {
        return;
    };
    let self_pid = std::process::id();
    if info.pid == self_pid {
        tracing::info!(
            pid = info.pid,
            "daemon.lock points to self (in-process daemon); skip kill, only clear lock"
        );
        let _ = std::fs::remove_file(memex_dir().join("daemon.lock"));
        return;
    }
    if !is_process_alive(info.pid) {
        let _ = std::fs::remove_file(memex_dir().join("daemon.lock"));
        return;
    }
    if let Err(e) = Command::new("kill")
        .args(["-TERM", &info.pid.to_string()])
        .status()
    {
        tracing::warn!(pid = info.pid, error = %e, "failed to send SIGTERM to daemon");
    }
    std::thread::sleep(std::time::Duration::from_millis(800));
    if is_process_alive(info.pid)
        && let Err(e) = Command::new("kill")
            .args(["-KILL", &info.pid.to_string()])
            .status()
    {
        tracing::warn!(pid = info.pid, error = %e, "failed to send SIGKILL to daemon");
    }
    let _ = std::fs::remove_file(memex_dir().join("daemon.lock"));
}

fn http_health_ok(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
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
}

/// 返回 daemon 写 stdout 日志的绝对路径，方便 GUI 直接 `open` 这个文件。
/// Phase 2 后 daemon 是 in-process 跑在 Tauri 主进程里，没有独立 stdout 日志文件，
/// 该路径会指向一个不存在的位置；前端会优雅降级到"日志面板"。保留 API 是
/// 为了不打破前端，待 Phase 5 清理。
#[tauri::command]
pub fn daemon_log_path() -> String {
    memex_dir()
        .join("daemon.stdout.log")
        .to_string_lossy()
        .into_owned()
}

/// 返回当前 in-process daemon 的状态。
///
/// Phase 2 之后唯一可信的真相源是 `DaemonState`（内存里的句柄），lock 文件只是
/// 给 memex-cli 这种外部进程发现 port 用。所以这里优先读 state：
/// * `state.snapshot() == Some` → running=true，再用 HTTP `/health` 探活
///   （HTTP 没起来时 running 仍为 true，前端按 `http_ok` 区分"启动中"vs"就绪"）
/// * `state.snapshot() == None`  → daemon 还没起来或启动失败 → running=false
fn build_status_from_state(state: &DaemonState) -> DaemonStatus {
    let snapshot = match tauri::async_runtime::block_on(state.snapshot()) {
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
    let http_ok = http_health_ok(snapshot.port);
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
    Ok(build_status_from_state(&state))
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
    state.restart(memex_daemon::DEFAULT_PORT).await?;

    // axum::serve 是异步起的，restart 返回时 HTTP 未必立刻可用。轮询 20 次，
    // 每次 150ms，总共最多等 3s；命中就立即返回，超时也返回当前 state 的
    // 快照，让前端区分"启动中"和"未启动"。
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        let status = build_status_from_state(state);
        if status.http_ok {
            return Ok(status);
        }
    }
    Ok(build_status_from_state(state))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// 把 `MEMEX_HOME` 重定向到临时目录，避免污染真实 ~/.memex。
    /// 调用方需用 #[serial(memex_home)] 标记测试，serial_test 会在所有共用
    /// 此 key 的测试间串行化（跨 module 也有效）。
    fn with_temp_memex<F: FnOnce(&std::path::Path)>(f: F) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prev = std::env::var("MEMEX_HOME").ok();
        // SAFETY: 由 #[serial(memex_home)] 串行化。
        unsafe { std::env::set_var("MEMEX_HOME", tmp.path()) };
        f(tmp.path());
        match prev {
            Some(v) => unsafe { std::env::set_var("MEMEX_HOME", v) },
            None => unsafe { std::env::remove_var("MEMEX_HOME") },
        }
    }

    fn write_lock(dir: &std::path::Path, pid: u32) {
        let info = LockInfo {
            pid,
            port: 0,
            started_at: "2026-06-09T00:00:00Z".into(),
        };
        std::fs::write(
            dir.join("daemon.lock"),
            serde_json::to_string(&info).unwrap(),
        )
        .unwrap();
    }

    /// lock 文件不存在时 stop 是纯 no-op，不应 panic。
    #[test]
    #[serial(memex_home)]
    fn stop_no_op_when_no_lock() {
        with_temp_memex(|dir| {
            stop_daemon_blocking();
            assert!(!dir.join("daemon.lock").exists());
        });
    }

    /// lock 存在但 pid 已死时，应该把过期 lock 清掉，不向不存在进程发信号。
    /// 用 PID 999999（macOS PID_MAX 是 99999，远超此值 → kill -0 必定失败）。
    #[test]
    #[serial(memex_home)]
    fn stop_cleans_stale_lock() {
        with_temp_memex(|dir| {
            write_lock(dir, 999_999);
            let lock = dir.join("daemon.lock");
            assert!(lock.exists());
            stop_daemon_blocking();
            assert!(!lock.exists(), "stale lock should be deleted");
        });
    }

    /// 启动一个 `sleep 30` 子进程，写 lock 指向它，调 stop_daemon_blocking
    /// 应该 SIGTERM 把它杀掉，并清 lock。这里只 assert lock 被清 + 信号已
    /// 到（用 wait_timeout 验证子进程已退出），不去看 zombie 状态。
    #[test]
    #[serial(memex_home)]
    fn stop_signals_running_child_and_cleans_lock() {
        with_temp_memex(|dir| {
            let mut child = Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("spawn sleep");
            let pid = child.id();
            write_lock(dir, pid);

            stop_daemon_blocking();

            // wait 一定要回收 child 否则它在测试结束后成 zombie。stop 内部
            // 已经发了 SIGTERM / SIGKILL，wait 应快速返回。
            let exit_status = child.wait().expect("wait child");
            assert!(
                !exit_status.success(),
                "killed child should not exit cleanly",
            );
            assert!(
                !dir.join("daemon.lock").exists(),
                "lock should be removed after stop",
            );
        });
    }
}
