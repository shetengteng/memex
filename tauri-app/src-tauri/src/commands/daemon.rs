use std::path::PathBuf;
use std::process::{Command, Stdio};

use memex_core::memex_dir;
use serde::{Deserialize, Serialize};

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
/// fire-and-forget：信号发了 + lock 清了即可，不阻塞等内核 reap zombie。
pub(crate) fn stop_daemon_blocking() {
    let Some(info) = read_lock() else {
        return;
    };
    if !is_process_alive(info.pid) {
        let _ = std::fs::remove_file(memex_dir().join("daemon.lock"));
        return;
    }
    let _ = Command::new("kill")
        .args(["-TERM", &info.pid.to_string()])
        .status();
    std::thread::sleep(std::time::Duration::from_millis(800));
    if is_process_alive(info.pid) {
        let _ = Command::new("kill")
            .args(["-KILL", &info.pid.to_string()])
            .status();
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

fn find_daemon_binary() -> Option<PathBuf> {
    // 优先用跟 menubar 主程序同目录的二进制；
    // bundle 打包出来的目录结构就是这样放的。
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let p = parent.join("memex-daemon");
            if p.exists() {
                return Some(p);
            }
        }
    }
    // 退而求其次：从 PATH 里找。
    if let Ok(out) = Command::new("which").arg("memex-daemon").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
    }
    None
}

/// 返回 daemon 写 stdout 日志的绝对路径，方便 GUI 直接 `open` 这个文件。
/// 该路径与 `memex-daemon` 自身写入逻辑保持一致（`~/.memex/daemon.stdout.log`）。
#[tauri::command]
pub fn daemon_log_path() -> String {
    memex_dir()
        .join("daemon.stdout.log")
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
pub async fn daemon_status() -> Result<DaemonStatus, String> {
    let info = read_lock();
    let mut status = DaemonStatus {
        running: false,
        pid: None,
        port: None,
        http_ok: false,
        started_at: None,
    };
    if let Some(info) = info {
        let alive = is_process_alive(info.pid);
        let http = if alive {
            http_health_ok(info.port)
        } else {
            false
        };
        status.running = alive;
        status.pid = Some(info.pid);
        status.port = Some(info.port);
        status.http_ok = http;
        status.started_at = Some(info.started_at);
    }
    Ok(status)
}

#[tauri::command]
pub async fn daemon_restart() -> Result<DaemonStatus, String> {
    // 复用 stop_daemon_blocking：保证停 daemon 的 TERM → KILL 顺序、超时、
    // lock 清理逻辑只有一份实现。
    let _ = stop_daemon_blocking();

    let bin = find_daemon_binary()
        .ok_or_else(|| "在 app 同目录和 PATH 上都找不到 memex-daemon 可执行文件".to_string())?;

    Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 daemon 失败（{}）：{}", bin.display(), e))?;

    // 等 daemon 写 lock 文件、绑端口。
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        if let Some(info) = read_lock() {
            if is_process_alive(info.pid) && http_health_ok(info.port) {
                return Ok(DaemonStatus {
                    running: true,
                    pid: Some(info.pid),
                    port: Some(info.port),
                    http_ok: true,
                    started_at: Some(info.started_at),
                });
            }
        }
    }
    // 即使 HTTP 还没就绪，也把已知的状态返回给 UI，
    // 让它能显示"启动中"而不是"未运行"。
    Ok(daemon_status().await.unwrap_or(DaemonStatus {
        running: false,
        pid: None,
        port: None,
        http_ok: false,
        started_at: None,
    }))
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
